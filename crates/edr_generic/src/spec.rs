use edr_eth::result::HaltReason;
use edr_eth::{
    block::{Header, PartialHeader},
    chain_spec::{EthHeaderConstants, L1ChainSpec},
    eips::eip1559::{BaseFeeParams, ConstantBaseFeeParams},
    SpecId,
};
use edr_evm::{
    chain_spec::{BlockEnvConstructor, ChainSpec},
    hardfork::Activations,
    transaction::TransactionError,
};
use edr_provider::time::TimeSinceEpoch;
use edr_provider::{ProviderSpec, TransactionFailureReason};
use revm_primitives::{BlockEnv, EvmWiring, InvalidTransaction, TransactionValidation};

use crate::GenericChainSpec;

impl EvmWiring for GenericChainSpec {
    type Block = revm_primitives::BlockEnv;

    type Hardfork = revm_primitives::SpecId;

    type HaltReason = revm_primitives::HaltReason;

    type Transaction = crate::transaction::SignedWithFallbackToPostEip155;
}

impl revm::EvmWiring for GenericChainSpec {
    type Context = ();

    fn handler<'evm, EXT, DB>(hardfork: Self::Hardfork) -> revm::EvmHandler<'evm, Self, EXT, DB>
    where
        DB: revm::Database,
    {
        revm::EvmHandler::mainnet_with_spec(hardfork)
    }
}

impl EthHeaderConstants for GenericChainSpec {
    const BASE_FEE_PARAMS: BaseFeeParams<Self> =
        BaseFeeParams::Constant(ConstantBaseFeeParams::ethereum());

    const MIN_ETHASH_DIFFICULTY: u64 = 131072;
}

impl BlockEnvConstructor<GenericChainSpec, Header> for BlockEnv {
    fn new_block_env(header: &Header, hardfork: SpecId) -> Self {
        BlockEnvConstructor::<L1ChainSpec, Header>::new_block_env(header, hardfork)
    }
}

impl BlockEnvConstructor<GenericChainSpec, PartialHeader> for BlockEnv {
    fn new_block_env(header: &PartialHeader, hardfork: SpecId) -> Self {
        BlockEnvConstructor::<L1ChainSpec, PartialHeader>::new_block_env(header, hardfork)
    }
}

impl ChainSpec for GenericChainSpec {
    type ReceiptBuilder = crate::receipt::execution::Builder;
    type RpcBlockConversionError = crate::rpc::block::ConversionError<Self>;
    type RpcReceiptConversionError = crate::rpc::receipt::ConversionError;
    type RpcTransactionConversionError = crate::rpc::transaction::ConversionError;

    fn cast_transaction_error<BlockchainErrorT, StateErrorT>(
        error: <Self::Transaction as TransactionValidation>::ValidationError,
    ) -> TransactionError<Self, BlockchainErrorT, StateErrorT> {
        // Can't use L1ChainSpec impl here as the TransactionError is generic
        // over the specific chain spec rather than just the validation error.
        // Instead, we copy the impl here.
        match error {
            InvalidTransaction::LackOfFundForMaxFee { fee, balance } => {
                TransactionError::LackOfFundForMaxFee { fee, balance }
            }
            remainder => TransactionError::InvalidTransaction(remainder),
        }
    }

    fn chain_hardfork_activations(chain_id: u64) -> Option<&'static Activations<Self>> {
        L1ChainSpec::chain_hardfork_activations(chain_id).map(Activations::as_chain_spec)
    }

    fn chain_name(chain_id: u64) -> Option<&'static str> {
        L1ChainSpec::chain_name(chain_id)
    }
}

// impl<TimerT: Clone + TimeSinceEpoch> ProviderSpec<TimerT> for GenericChainSpec {
//     type PooledTransaction = edr_eth::transaction::pooled::PooledTransaction;
//     type TransactionRequest = edr_eth::transaction::Request;

//     fn cast_halt_reason(reason: Self::HaltReason) -> TransactionFailureReason<Self> {
//         match reason {
//             HaltReason::CreateContractSizeLimit => {
//                 TransactionFailureReason::CreateContractSizeLimit
//             }
//             HaltReason::OpcodeNotFound | HaltReason::InvalidFEOpcode => {
//                 TransactionFailureReason::OpcodeNotFound
//             }
//             HaltReason::OutOfGas(error) => TransactionFailureReason::OutOfGas(error),
//             remainder => TransactionFailureReason::Inner(remainder),
//         }
//     }
// }
