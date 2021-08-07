// Copyright 2021 The Rethnet Authors.
// Licensed under the Apache License, Version 2.0.

mod fork;
mod vm;

#[cfg(feature = "nodejs")]
use napi::{Env, JsObject, Result};
#[cfg(feature = "nodejs")]
use napi_derive::*;
pub use vm::VirtualMachine;

#[cfg(feature = "nodejs")]
#[module_exports]
fn init(mut exports: JsObject, env: Env) -> Result<()> {
  // export SDK constant
  exports.set_named_property(
    "RETHNET_SDK_VERSION",
    env.create_string(env!("CARGO_PKG_VERSION"))?,
  )?;

  // export SDK classes to NodeJS
  exports.set_named_property("VirtualMachine", VirtualMachine::define_js_class(&env)?)?;

  Ok(())
}

#[cfg(test)]
mod tests {
  #[test]
  fn it_works() {
    assert_eq!(2 + 2, 4);
  }
}
