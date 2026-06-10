use std::sync::{mpsc::channel, Arc};

use edr_primitives::{Address, Bytes};
use napi::{
    bindgen_prelude::{FnArgs, Function, Promise, Uint8Array},
    threadsafe_function::{ThreadsafeCallContext, ThreadsafeFunction, ThreadsafeFunctionCallMode},
    tokio::runtime,
    Env, Status,
};
use napi_derive::napi;

use crate::cast::TryCast;

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
    Status,
    false,
    true,
    0,
>;

#[derive(Clone)]
pub struct CallOverrideCallback {
    call_override_callback_fn: Arc<CallOverrideTsfn>,
    runtime: runtime::Handle,
}

impl CallOverrideCallback {
    pub fn new(
        _env: &Env,
        call_override_callback: Function<
            '_,
            FnArgs<(Uint8Array, Uint8Array)>,
            Promise<Option<CallOverrideResult>>,
        >,
        runtime: runtime::Handle,
    ) -> napi::Result<Self> {
        let call_override_callback_fn = call_override_callback
            .build_threadsafe_function::<CallOverrideCall>()
            // Maintain a weak reference to the function to avoid blocking
            // the event loop from exiting.
            .weak::<true>()
            .build_callback(|ctx: ThreadsafeCallContext<CallOverrideCall>| {
                let address = Uint8Array::from(ctx.value.contract_address.to_vec());
                let data = Uint8Array::from(ctx.value.data.to_vec());

                Ok(FnArgs {
                    data: (address, data),
                })
            })?;

        Ok(Self {
            call_override_callback_fn: Arc::new(call_override_callback_fn),
            runtime,
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
            // sender.
            move |result: napi::Result<Promise<Option<CallOverrideResult>>>, _env: Env| {
                match result {
                    Ok(promise) => {
                        runtime.spawn(async move {
                            let result = match promise.await {
                                Ok(value) => value.try_cast(),
                                Err(error) => Err(error),
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
                        sender.send(Err(error)).map_err(|_error| {
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

        assert_eq!(status, Status::Ok, "Call override callback failed");

        // `SyncCallOverride` has no error path, and silently returning `None`
        // would alter `eth_call` results, so a failing JS callback must fail
        // loudly. This runs on a tokio blocking thread, so the panic is
        // caught by the runtime and surfaces as a JSON-RPC internal error
        // rather than aborting the process.
        receiver
            .recv()
            .expect("Channel can only close if the threadsafe call was dropped without running")
            .unwrap_or_else(|error| panic!("Call override callback failed: {error}"))
    }
}
