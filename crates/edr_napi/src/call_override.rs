use std::sync::{mpsc::channel, Arc};

use edr_primitives::{Address, Bytes};
use napi::{
    bindgen_prelude::{FnArgs, Function, Object, Promise, Uint8Array},
    threadsafe_function::{ThreadsafeCallContext, ThreadsafeFunction, ThreadsafeFunctionCallMode},
    tokio::runtime,
    Env, Status,
};
use napi_derive::napi;

use crate::{
    callback::{self, OnOwnerCollected, PendingAttachment, PendingUnroot},
    cast::TryCast,
    napi_error,
};

/// The result of executing a call override.
#[napi(object)]
pub struct CallOverrideResult {
    pub result: Uint8Array,
    pub should_revert: bool,
}

impl TryCast<Option<edr_provider::CallOverrideResult>> for Option<CallOverrideResult> {
    type Error = napi::Error;

    fn try_cast(self) -> Result<Option<edr_provider::CallOverrideResult>, Self::Error> {
        match self {
            None => Ok(None),
            Some(result) => Ok(Some(edr_provider::CallOverrideResult {
                output: Bytes::copy_from_slice(&result.result),
                should_revert: result.should_revert,
            })),
        }
    }
}

struct CallOverrideCall {
    contract_address: Address,
    data: Bytes,
}

type CallOverrideTsfn = ThreadsafeFunction<
    CallOverrideCall,
    Promise<Option<CallOverrideResult>>,
    FnArgs<(Uint8Array, Uint8Array)>,
    /* ErrorStatus */ Status,
    /* CalleeHandled */ false,
    /* Weak */ { callback::EVENT_LOOP_UNREFERENCED },
    /* MaxQueueSize */ 0,
>;

#[derive(Clone)]
pub struct CallOverrideCallback {
    call_override_callback_fn: Arc<CallOverrideTsfn>,
    runtime: runtime::Handle,
}

/// What [`CallOverrideCallback::resolve`] produces.
pub struct ResolvedCallOverride {
    /// What the provider calls.
    pub callback: CallOverrideCallback,
    /// Completes on the JavaScript thread once the provider holds `callback`.
    pub attachment: PendingAttachment,
}

impl CallOverrideCallback {
    /// Builds the threadsafe function the provider calls, along with the
    /// attachment that will make `owner` own the consumer's callback.
    ///
    /// See [`crate::callback`] for why the callback is kept out of the
    /// threadsafe function.
    pub fn resolve(
        env: &Env,
        owner: &Object<'_>,
        call_override_callback: Function<
            '_,
            FnArgs<(Uint8Array, Uint8Array)>,
            Promise<Option<CallOverrideResult>>,
        >,
        runtime: runtime::Handle,
    ) -> napi::Result<ResolvedCallOverride> {
        let PendingUnroot {
            built: call_override_callback_fn,
            attachment,
        } = callback::unroot_pending(
            env,
            owner,
            call_override_callback,
            "callOverride",
            // The callback's result decides an `eth_call`, and returning
            // `undefined` would silently make it `None`. A late call
            // therefore fails with the callback's name, which panics the
            // provider's thread; see `call_override` below. Only a collected
            // owner reaches this, so the consumer no longer holds the
            // provider.
            OnOwnerCollected::Throw,
            |trampoline| {
                trampoline
                    .build_threadsafe_function::<CallOverrideCall>()
                    // Unreferenced from the event loop; see the constant's
                    // docs.
                    .weak::<{ callback::EVENT_LOOP_UNREFERENCED }>()
                    .build_callback(|ctx: ThreadsafeCallContext<CallOverrideCall>| {
                        let address = Uint8Array::from(ctx.value.contract_address.to_vec());
                        let data = Uint8Array::from(ctx.value.data.to_vec());

                        Ok(FnArgs {
                            data: (address, data),
                        })
                    })
            },
        )?;

        Ok(ResolvedCallOverride {
            callback: Self {
                call_override_callback_fn: Arc::new(call_override_callback_fn),
                runtime,
            },
            attachment,
        })
    }

    pub fn call_override(
        &self,
        contract_address: Address,
        data: Bytes,
    ) -> Option<edr_provider::CallOverrideResult> {
        let (sender, receiver) = channel();

        let runtime = self.runtime.clone();
        let status = self.call_override_callback_fn.call_with_return_value(
            CallOverrideCall {
                contract_address,
                data,
            },
            ThreadsafeFunctionCallMode::Blocking,
            // Always send through the channel — including the `Err` cases
            // when the JS callback throws synchronously or its promise
            // rejects — so the `recv` below can't be left with a dropped
            // sender. Errors cross the channel as `String`: a `napi::Error`
            // from a JS throw or rejection owns a `napi_ref` that must not
            // drop off the JS thread (see `crate::napi_error`).
            move |result: napi::Result<Promise<Option<CallOverrideResult>>>, _env: Env| {
                match result {
                    Ok(promise) => {
                        runtime.spawn(async move {
                            let result: Result<Option<edr_provider::CallOverrideResult>, String> =
                                match promise.await {
                                    Ok(value) => value
                                        .try_cast()
                                        .map_err(|error: napi::Error| error.to_string()),
                                    Err(error) => Err(napi_error::reason_and_forget(error)),
                                };
                            sender.send(result).map_err(|_error| {
                                napi::Error::new(
                                    Status::GenericFailure,
                                    "Failed to send result from call_override_callback",
                                )
                            })
                        });
                    }
                    Err(error) => {
                        // On the JS thread; dropping the error here is safe.
                        sender.send(Err(error.to_string())).map_err(|_error| {
                            napi::Error::new(
                                Status::GenericFailure,
                                "Failed to send result from call_override_callback",
                            )
                        })?;
                    }
                }
                Ok(())
            },
        );

        // Distinct from the callback-failure panic below: a non-`Ok` status
        // means the threadsafe call itself was not scheduled (e.g. `Closing`
        // during environment teardown), not that the user's callback failed.
        assert_eq!(status, Status::Ok, "Call override threadsafe call failed");

        // `SyncCallOverride` has no error path, and silently returning `None`
        // would alter `eth_call` results, so a failing JS callback must fail
        // loudly. This runs on the provider's thread, so the panic terminates
        // it and every subsequent request reports `UnexpectedTermination`.
        receiver
            .recv()
            .expect("Channel can only close if the threadsafe call was dropped without running")
            .unwrap_or_else(|error| panic!("Call override callback failed: {error}"))
    }
}
