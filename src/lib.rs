// Copyright 2021 The Rethnet Authors.
// Licensed under the Apache License, Version 2.0.

use napi::{Env, JsObject, Result};
use napi_derive::*;

#[module_exports]
fn init(mut exports: JsObject, env: Env) -> Result<()> {
  exports.set_named_property(
    "RETHNET_SDK_VERSION",
    env.create_string(env!("CARGO_PKG_VERSION"))?,
  )?;
  Ok(())
}

#[cfg(test)]
mod tests {
  #[test]
  fn it_works() {
    assert_eq!(2 + 2, 4);
  }
}
