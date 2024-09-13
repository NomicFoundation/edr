use std::marker::PhantomData;

use alloy_rlp::RlpEncodable;
use revm::Database;
pub use revm_primitives::EvmWiring;

use crate::{
    eips::eip1559::{BaseFeeParams, ConstantBaseFeeParams},
    transaction,
};

/// The chain specification for Ethereum Layer 1.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, RlpEncodable)]
pub struct L1ChainSpec<DatabaseT: Database, ExternalContextT> {
    phantom: PhantomData<(DatabaseT, ExternalContextT)>,
}

impl<DatabaseT: Database, ExternalContextT> EvmWiring for L1ChainSpec<DatabaseT, ExternalContextT> {
    type ExternalContext = ExternalContextT;

    type ChainContext = ();

    type Database = DatabaseT;

    type Block = revm_primitives::BlockEnv;

    type Hardfork = revm_primitives::SpecId;

    type HaltReason = revm_primitives::HaltReason;

    type Transaction = transaction::Signed;
}

impl<DatabaseT: Database, ExternalContextT> revm::EvmWiring
    for L1ChainSpec<DatabaseT, ExternalContextT>
{
    fn handler<'evm>(hardfork: Self::Hardfork) -> revm::EvmHandler<'evm, Self> {
        revm::EvmHandler::mainnet_with_spec(hardfork)
    }
}

/// Constants for constructing Ethereum headers.
pub trait EthHeaderConstants: revm_primitives::EvmWiring<Hardfork: 'static + PartialOrd> {
    /// Parameters for the EIP-1559 base fee calculation.
    const BASE_FEE_PARAMS: BaseFeeParams<Self::Hardfork>;

    /// The minimum difficulty for the Ethash proof-of-work algorithm.
    const MIN_ETHASH_DIFFICULTY: u64;
}

impl<DatabaseT: Database, ExternalContextT> EthHeaderConstants
    for L1ChainSpec<DatabaseT, ExternalContextT>
{
    const BASE_FEE_PARAMS: BaseFeeParams<Self::Hardfork> =
        BaseFeeParams::Constant(ConstantBaseFeeParams::ethereum());

    const MIN_ETHASH_DIFFICULTY: u64 = 131072;
}
