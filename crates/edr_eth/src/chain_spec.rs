use std::marker::PhantomData;

use alloy_rlp::RlpEncodable;
use revm::Database;
pub use revm_primitives::EvmWiring;
use revm_primitives::{ChainSpec, InvalidTransaction, TransactionValidation};

use crate::{
    eips::eip1559::{BaseFeeParams, ConstantBaseFeeParams},
    transaction,
};

/// A wrapper around the EVM's wiring.
pub struct Wiring<ChainSpecT: ChainSpec, DatabaseT: Database, ExternalContextT> {
    _phantom: PhantomData<(ChainSpecT, DatabaseT, ExternalContextT)>,
}

impl<ChainSpecT: ChainSpec, DatabaseT: Database, ExternalContextT> EvmWiring
    for Wiring<ChainSpecT, DatabaseT, ExternalContextT>
{
    type ChainSpec = ChainSpecT;
    type ExternalContext = ExternalContextT;
    type Database = DatabaseT;
}

impl<ChainSpecT, DatabaseT, ExternalContextT> revm::EvmWiring
    for Wiring<ChainSpecT, DatabaseT, ExternalContextT>
where
    ChainSpecT:
        ChainSpec<Transaction: TransactionValidation<ValidationError: From<InvalidTransaction>>>,
    DatabaseT: Database,
{
    fn handler<'evm>(hardfork: Self::Hardfork) -> revm::EvmHandler<'evm, Self> {
        revm::EvmHandler::mainnet_with_spec(hardfork)
    }
}

/// The chain specification for Ethereum Layer 1.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, RlpEncodable)]
pub struct L1ChainSpec;

impl ChainSpec for L1ChainSpec {
    type ChainContext = ();
    type Block = revm_primitives::BlockEnv;
    type Transaction = transaction::Signed;
    type Hardfork = revm_primitives::SpecId;
    type HaltReason = revm_primitives::HaltReason;
}

/// Constants for constructing Ethereum headers.
pub trait EthHeaderConstants: ChainSpec<Hardfork: 'static + PartialOrd> {
    /// Parameters for the EIP-1559 base fee calculation.
    const BASE_FEE_PARAMS: BaseFeeParams<Self::Hardfork>;

    /// The minimum difficulty for the Ethash proof-of-work algorithm.
    const MIN_ETHASH_DIFFICULTY: u64;
}

impl EthHeaderConstants for L1ChainSpec {
    const BASE_FEE_PARAMS: BaseFeeParams<Self::Hardfork> =
        BaseFeeParams::Constant(ConstantBaseFeeParams::ethereum());

    const MIN_ETHASH_DIFFICULTY: u64 = 131072;
}
