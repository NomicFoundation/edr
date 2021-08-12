// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: Copyright 2021 contributors to the Rethnet project.

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


#[cfg(test)]
mod tests {}
