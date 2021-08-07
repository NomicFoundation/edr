// Copyright 2021 The Rethnet Authors.
// Licensed under the Apache License, Version 2.0.

use std::{error::Error, fmt::Display};

use crate::fork::Hardfork;

/// Configures the EVM by setting the desired hardfork (version) and network id
pub struct NetworkConfig {
  pub chain_id: i64,
  pub hardfork: Hardfork,
}

/// Configures the first block of the blockchain for the EVM.
pub struct GenesisBlockConfig;

/// Represents an instance of the EVM.
pub struct VirtualMachine {
  pub network: NetworkConfig,
  pub genesis: GenesisBlockConfig,
}

#[derive(Debug)]
pub struct VMError;

pub type Result<T> = std::result::Result<T, VMError>;

impl VirtualMachine {
  pub fn new(genesis: GenesisBlockConfig, network: NetworkConfig) -> Result<VirtualMachine> {
    Ok(VirtualMachine { genesis, network })
  }
}

impl Display for VMError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.write_str("VMError: TODO")
  }
}

impl Error for VMError {}

/// Exposes the EVM api to nodejs through N-API.
/// to use those bindings make sure that the "nodejs" feature is enabled.
#[cfg(feature = "nodejs")]
pub mod napi {
  use std::convert::{TryFrom, TryInto};

  use napi::{
    CallContext,
    Env,
    JsFunction,
    JsNumber,
    JsObject,
    JsString,
    JsUndefined,
    Property,
    PropertyAttributes,
    Result,
    Status,
  };
  use napi_derive::*;

  use super::{GenesisBlockConfig, NetworkConfig, VirtualMachine};

  impl TryFrom<JsObject> for NetworkConfig {
    type Error = napi::Error;

    fn try_from(value: JsObject) -> Result<NetworkConfig> {
      let chain_id: i64 = value
        .get_named_property::<JsNumber>("chainId")?
        .try_into()?;
      let hardfork: String = value
        .get_named_property::<JsString>("hardfork")?
        .into_utf8()?
        .into_owned()?;

      Ok(NetworkConfig {
        chain_id,
        hardfork: hardfork.as_str().try_into().map_err(|_| {
          napi::Error::new(
            Status::InvalidArg,
            "missing 'hardfork' propoerty".to_owned(),
          )
        })?,
      })
    }
  }

  impl TryFrom<JsObject> for GenesisBlockConfig {
    type Error = napi::Error;

    fn try_from(_value: JsObject) -> Result<GenesisBlockConfig> {
      Ok(GenesisBlockConfig)
    }
  }

  #[js_function(2)]
  fn vm_js_construct(ctx: CallContext) -> napi::Result<JsUndefined> {
    // those are the constructor arguments.
    // translate JS objects to rust native instances
    let network = NetworkConfig::try_from(ctx.get::<JsObject>(0)?)?;
    let genesis = GenesisBlockConfig::try_from(ctx.get::<JsObject>(1)?)?;

    // create a JS object and attach the native rust instance to it.
    // all the work should happen on the native side, and the N-API
    // wrapper should do only conversion between rust and JS types.
    // see https://github.com/nomiclabs/rethnet/issues/1
    let mut this: JsObject = ctx.this_unchecked();
    ctx
      .env
      .wrap(&mut this, VirtualMachine { genesis, network })?;

    // TODO: attach properties to "this"
    ctx.env.get_undefined()
  }

  #[js_function]
  fn vm_get_network(ctx: CallContext) -> Result<JsObject> {
    let this = ctx.this_unchecked::<JsObject>();
    let vm = ctx.env.unwrap::<VirtualMachine>(&this)?;
    let mut obj = ctx.env.create_object()?;

    let chain_id = ctx.env.create_int64(vm.network.chain_id)?;
    let hardfork = ctx
      .env
      .create_string(&format!("{:#?}", vm.network.hardfork))?;

    obj.set_named_property("chainId", chain_id)?;
    obj.set_named_property("hardfork", hardfork)?;

    Ok(obj)
  }

  #[js_function]
  fn vm_get_genesis(_ctx: CallContext) -> Result<JsObject> {
    todo!()
  }

  impl VirtualMachine {
    pub fn define_js_class(env: &Env) -> Result<JsFunction> {
      env.define_class(
        "VirtualMachine",
        vm_js_construct,
        &vec![
          Property::new(env, "network")?
            .with_getter(vm_get_network)
            .with_property_attributes(PropertyAttributes::Enumerable),
          Property::new(env, "genesis")?
            .with_getter(vm_get_genesis)
            .with_property_attributes(PropertyAttributes::Enumerable), // ro, set in constructor
        ],
      )
    }
  }
}

#[cfg(test)]
mod tests {}
