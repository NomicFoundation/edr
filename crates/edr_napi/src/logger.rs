use std::sync::{mpsc::channel, Arc};

use edr_napi_core::logger::LoggerError;
use edr_primitives::Bytes;
use napi::{
    bindgen_prelude::{FnArgs, Function, Uint8Array},
    threadsafe_function::{ThreadsafeCallContext, ThreadsafeFunctionCallMode},
    Env, Status,
};
use napi_derive::napi;

/// Configuration for the provider's logger.
#[napi(object)]
pub struct LoggerConfig<'env> {
    /// Whether to enable the logger.
    pub enable: bool,
    // `ts_type` declares `ArrayBuffer[]` to match Hardhat 2's typings; the
    // runtime value is a `Uint8Array[]`, which `Buffer.from(x)` accepts.
    #[napi(ts_type = "(inputs: ArrayBuffer[]) => string[]")]
    pub decode_console_log_inputs_callback: Function<'env, Vec<Uint8Array>, Vec<String>>,
    #[napi(ts_type = "(message: string, replace: boolean) => void")]
    pub print_line_callback: Function<'env, FnArgs<(String, bool)>, ()>,
}

impl LoggerConfig<'_> {
    /// Resolves the logger config, converting it to a
    /// `edr_napi_core::logger::Config`.
    pub fn resolve(self) -> napi::Result<edr_napi_core::logger::Config> {
        let decode_console_log_inputs_callback = self
            .decode_console_log_inputs_callback
            .build_threadsafe_function::<Vec<Bytes>>()
            // Maintain a weak reference to the function to avoid blocking
            // the event loop from exiting.
            .weak::<true>()
            .build_callback(|ctx: ThreadsafeCallContext<Vec<Bytes>>| {
                let inputs: Vec<Uint8Array> = ctx
                    .value
                    .into_iter()
                    .map(|input| Uint8Array::from(input.to_vec()))
                    .collect();
                Ok(inputs)
            })?;

        let decode_console_log_inputs_fn = Arc::new(move |console_log_inputs| {
            let (sender, receiver) = channel();

            // Always send — including the `Err` when the JS callback throws —
            // so the `recv` below can't be left with a dropped sender. The
            // `Err` is stringified here, on the JS thread: a JS-thrown
            // `napi::Error` owns a `napi_ref` that must not drop on the
            // receiving thread (see `crate::napi_error`).
            let status = decode_console_log_inputs_callback.call_with_return_value(
                console_log_inputs,
                ThreadsafeFunctionCallMode::Blocking,
                move |decoded_inputs: napi::Result<Vec<String>>, _env: Env| {
                    sender
                        .send(decoded_inputs.map_err(|error| error.to_string()))
                        .map_err(|_error| {
                            napi::Error::new(
                                Status::GenericFailure,
                                "Failed to send result from decode_console_log_inputs",
                            )
                        })
                },
            );
            if status != Status::Ok {
                return Err(LoggerError::DecodeConsoleLogInputs(format!(
                    "Threadsafe call failed with status {status:?}"
                )));
            }

            // The closure always sends when invoked, so the channel can only
            // be closed if the threadsafe call was dropped without running it
            // (e.g. during environment teardown).
            receiver
                .recv()
                .map_err(|_error| {
                    LoggerError::DecodeConsoleLogInputs(
                        "Callback was dropped before returning a result".to_owned(),
                    )
                })?
                .map_err(LoggerError::DecodeConsoleLogInputs)
        });

        let print_line_callback = self
            .print_line_callback
            .build_threadsafe_function::<(String, bool)>()
            // Maintain a weak reference to the function to avoid blocking
            // the event loop from exiting.
            .weak::<true>()
            .build_callback(|ctx: ThreadsafeCallContext<(String, bool)>| {
                Ok(FnArgs { data: ctx.value })
            })?;

        let print_line_fn = Arc::new(move |message, replace| {
            let (sender, receiver) = channel();

            // Forward the `Err` so a throwing `printLineCallback` surfaces as
            // a `LoggerError` — stringified on the JS thread, since a
            // JS-thrown `napi::Error` owns a `napi_ref` that must not drop on
            // the receiving thread (see `crate::napi_error`).
            let status = print_line_callback.call_with_return_value(
                (message, replace),
                ThreadsafeFunctionCallMode::Blocking,
                move |result: napi::Result<()>, _env: Env| {
                    sender
                        .send(result.map_err(|error| error.to_string()))
                        .map_err(|_error| {
                            napi::Error::new(
                                Status::GenericFailure,
                                "Failed to send result from print_line_callback",
                            )
                        })
                },
            );
            if status != Status::Ok {
                return Err(LoggerError::PrintLine(format!(
                    "Threadsafe call failed with status {status:?}"
                )));
            }

            receiver
                .recv()
                .map_err(|_error| {
                    LoggerError::PrintLine(
                        "Callback was dropped before returning a result".to_owned(),
                    )
                })?
                .map_err(LoggerError::PrintLine)
        });

        Ok(edr_napi_core::logger::Config {
            enable: self.enable,
            decode_console_log_inputs_fn,
            print_line_fn,
        })
    }
}
