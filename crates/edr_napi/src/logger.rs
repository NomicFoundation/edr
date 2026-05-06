use std::sync::{mpsc::channel, Arc};

use edr_napi_core::logger::LoggerError;
use edr_primitives::Bytes;
use napi::{
    bindgen_prelude::{Buffer, FnArgs, Function},
    threadsafe_function::{ThreadsafeCallContext, ThreadsafeFunctionCallMode},
    Env, Status,
};
use napi_derive::napi;

#[napi(object)]
pub struct LoggerConfig<'env> {
    /// Whether to enable the logger.
    pub enable: bool,
    #[napi(ts_type = "(inputs: Buffer[]) => string[]")]
    pub decode_console_log_inputs_callback: Function<'env, Vec<Buffer>, Vec<String>>,
    #[napi(ts_type = "(message: string, replace: boolean) => void")]
    pub print_line_callback: Function<'env, FnArgs<(String, bool)>, ()>,
}

impl LoggerConfig<'_> {
    /// Resolves the logger config, converting it to a
    /// `edr_napi_core::logger::Config`.
    pub fn resolve(self, _env: &Env) -> napi::Result<edr_napi_core::logger::Config> {
        let decode_console_log_inputs_callback = self
            .decode_console_log_inputs_callback
            .build_threadsafe_function::<Vec<Bytes>>()
            .weak::<true>()
            .build_callback(|ctx: ThreadsafeCallContext<Vec<Bytes>>| {
                let inputs: Vec<Buffer> = ctx
                    .value
                    .into_iter()
                    .map(|input| Buffer::from(input.to_vec()))
                    .collect();
                Ok(inputs)
            })?;

        let decode_console_log_inputs_fn = Arc::new(move |console_log_inputs| {
            let (sender, receiver) = channel();

            let status = decode_console_log_inputs_callback.call_with_return_value(
                console_log_inputs,
                ThreadsafeFunctionCallMode::Blocking,
                move |decoded_inputs: napi::Result<Vec<String>>, _env: Env| {
                    let decoded_inputs = decoded_inputs?;
                    sender.send(decoded_inputs).map_err(|_error| {
                        napi::Error::new(
                            Status::GenericFailure,
                            "Failed to send result from decode_console_log_inputs",
                        )
                    })
                },
            );
            assert_eq!(status, Status::Ok);

            receiver
                .recv()
                .expect("Receive can only fail if the channel is closed")
        });

        let print_line_callback = self
            .print_line_callback
            .build_threadsafe_function::<(String, bool)>()
            .weak::<true>()
            .build_callback(|ctx: ThreadsafeCallContext<(String, bool)>| {
                Ok(FnArgs { data: ctx.value })
            })?;

        let print_line_fn = Arc::new(move |message, replace| {
            let (sender, receiver) = channel();

            let status = print_line_callback.call_with_return_value(
                (message, replace),
                ThreadsafeFunctionCallMode::Blocking,
                move |result: napi::Result<()>, _env: Env| {
                    sender.send(result.is_ok()).map_err(|_error| {
                        napi::Error::new(
                            Status::GenericFailure,
                            "Failed to send result from print_line_callback",
                        )
                    })
                },
            );

            let succeeded = receiver.recv().unwrap_or(false);

            if status == napi::Status::Ok && succeeded {
                Ok(())
            } else {
                Err(LoggerError::PrintLine)
            }
        });

        Ok(edr_napi_core::logger::Config {
            enable: self.enable,
            decode_console_log_inputs_fn,
            print_line_fn,
        })
    }
}
