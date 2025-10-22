//! Configuration types for the EVM.

use edr_chain_spec::EvmSpecId;

use crate::CfgEnv;

/// Configuration options for the EVM.
#[derive(Clone, Debug)]
pub struct EvmConfig {
    /// Chain ID of the EVM. Used in CHAINID opcode and transaction's chain ID
    /// check.
    ///
    /// Chain ID was introduced in EIP-155.
    pub chain_id: u64,
    /// EIP-3607 rejects transactions from senders with deployed code
    ///
    /// In development, it can be desirable to simulate calls from contracts,
    /// which this setting allows.
    ///
    /// By default, it is set to `false`.
    pub disable_eip3607: bool,
    /// Contract code size limit override.
    ///
    /// If None, the limit will be determined by the hardfork (EIP-170 or
    /// EIP-7907) at runtime. If Some, this specific limit will be used
    /// regardless of hardfork.
    pub limit_contract_code_size: Option<usize>,
}

impl EvmConfig {
    /// Converts the EVM configuration into a `CfgEnv` for the specified
    /// hardfork.
    pub fn to_cfg_env<HardforkT: Into<EvmSpecId>>(&self, hardfork: HardforkT) -> CfgEnv<HardforkT> {
        let mut cfg_env = CfgEnv::new_with_spec(hardfork);
        cfg_env.chain_id = self.chain_id;
        cfg_env.disable_eip3607 = self.disable_eip3607;
        cfg_env.limit_contract_code_size = self.limit_contract_code_size;

        cfg_env
    }
}
