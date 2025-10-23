use crate::inspector::Cheatcodes;
use alloy_primitives::{Address, Bytes, U256};
use revm::context::CfgEnv;
use revm::context::result::HaltReasonTr;
use revm::interpreter::{CreateInputs};
use revm::Journal;
use foundry_evm_core::backend::CheatcodeBackend;
use foundry_evm_core::evm_context::{BlockEnvTr, ChainContextTr, EvmBuilderTrait, HardforkTr, TransactionEnvTr, TransactionErrorTrait};

/// Common behaviour of legacy and EOF create inputs.
pub(crate) trait CommonCreateInput {
    fn caller(&self) -> Address;
    fn gas_limit(&self) -> u64;
    fn value(&self) -> U256;
    fn init_code(&self) -> Bytes;
    fn set_caller(&mut self, caller: Address);
    fn allow_cheatcodes<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        TransactionErrorT: TransactionErrorTrait,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
        >,
    >(&self,
      cheatcodes: &mut Cheatcodes<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT, TransactionErrorT>,
      ecx: &mut revm::context::Context<BlockT, TxT, CfgEnv<HardforkT>, DatabaseT, Journal<DatabaseT>, ChainContextT>) -> Address;
}

impl CommonCreateInput for &mut CreateInputs {
    fn caller(&self) -> Address {
        self.caller
    }
    fn gas_limit(&self) -> u64 {
        self.gas_limit
    }
    fn value(&self) -> U256 {
        self.value
    }
    fn init_code(&self) -> Bytes {
        self.init_code.clone()
    }
    fn set_caller(&mut self, caller: Address) {
        self.caller = caller;
    }
    fn allow_cheatcodes<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        TransactionErrorT: TransactionErrorTrait,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
        >,
    >(&self,
      cheatcodes: &mut Cheatcodes<BlockT, TxT, ChainContextT, EvmBuilderT, HaltReasonT, HardforkT, TransactionErrorT>,
      ecx: &mut revm::context::Context<BlockT, TxT, CfgEnv<HardforkT>, DatabaseT, Journal<DatabaseT>, ChainContextT>) -> Address {
        let old_nonce = ecx
            .journaled_state
            .state
            .get(&self.caller)
            .map(|acc| acc.info.nonce)
            .unwrap_or_default();
        let created_address = self.created_address(old_nonce);
        cheatcodes.allow_cheatcodes_on_create(ecx, self.caller, created_address);
        created_address
    }
}
