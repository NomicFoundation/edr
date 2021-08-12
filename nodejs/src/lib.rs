// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: Copyright 2021 contributors to the Rethnet project.

mod vm;

use napi_derive::*;
use napi::{Env, JsObject, Result};

#[module_exports]
fn init(mut exports: JsObject, env: Env) -> Result<()> {
  // export SDK constant
  exports.set_named_property(
    "RETHNET_SDK_VERSION",
    env.create_string(env!("CARGO_PKG_VERSION"))?,
  )?;

  // export SDK classes to NodeJS
  exports.set_named_property("VirtualMachine", 
    vm::define_virtual_machine_class(&env)?)?;

  Ok(())
}