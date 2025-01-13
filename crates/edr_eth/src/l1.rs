use alloy_rlp::RlpEncodable;
use revm_specification::hardfork::SpecId;
pub use revm_wiring::{default::block::BlockEnv, result::HaltReason};

use crate::{
    eips::eip1559::{BaseFeeParams, ConstantBaseFeeParams},
    spec::{ChainSpec, EthHeaderConstants},
    transaction,
};

/// The chain specification for Ethereum Layer 1.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, RlpEncodable)]
pub struct L1ChainSpec;

impl ChainSpec for L1ChainSpec {
    type BlockEnv = BlockEnv;
    type Context = ();
    type HaltReason = HaltReason;
    type Hardfork = Hardfork;
    type SignedTransaction = transaction::Signed;
}

impl EthHeaderConstants for L1ChainSpec {
    const BASE_FEE_PARAMS: BaseFeeParams<Self::Hardfork> =
        BaseFeeParams::Constant(ConstantBaseFeeParams::ethereum());

    const MIN_ETHASH_DIFFICULTY: u64 = 131072;
}

/// Hardfork names.
pub mod name {
    /// Byzantium hardfork name.
    pub const BYZANTIUM: &str = "byzantium";
    /// Constantinople hardfork name.
    pub const CONSTANTINOPLE: &str = "constantinople";
    /// Petersburg hardfork name.
    pub const PETERSBURG: &str = "petersburg";
    /// Istanbul hardfork name.
    pub const ISTANBUL: &str = "istanbul";
    /// Muir Glacier hardfork name.
    pub const MUIR_GLACIER: &str = "muirGlacier";
    /// Berlin hardfork name.
    pub const BERLIN: &str = "berlin";
    /// London hardfork name.
    pub const LONDON: &str = "london";
    /// Arrow Glacier hardfork name.
    pub const ARROW_GLACIER: &str = "arrowGlacier";
    /// Gray Glacier hardfork name.
    pub const GRAY_GLACIER: &str = "grayGlacier";
    /// Merge hardfork name.
    pub const MERGE: &str = "merge";
    /// Shanghai hardfork name.
    pub const SHANGHAI: &str = "shanghai";
    /// Cancun hardfork name.
    pub const CANCUN: &str = "cancun";
    /// Identifier for the latest hardfork.
    pub const LATEST: &str = "latest";
}

/// L1 hardfork type with ONLY supported hardforks.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(u16)]
pub enum Hardfork {
    /// Byzantium           4370000
    Byzantium = 6,
    /// Constantinople      7280000 is overwritten with PETERSBURG
    Constantinople = 7,
    /// Petersburg          7280000
    Petersburg = 8,
    /// Istanbul            9069000
    Istanbul = 9,
    /// Muir Glacier        9200000
    MuirGlacier = 10,
    /// Berlin              12244000
    Berlin = 11,
    /// London              12965000
    London = 12,
    /// Arrow Glacier       13773000
    ArrowGlacier = 13,
    /// Gray Glacier        15050000
    GrayGlacier = 14,
    /// Paris/Merge         15537394 (TTD: 58750000000000000000000)
    Merge = 15,
    /// Shanghai            17034870 (Timestamp: 1681338455)
    Shanghai = 16,
    /// Cancun              19426587 (Timestamp: 1710338135)
    Cancun = 17,
    /// Latest hardfork
    #[default]
    Latest = u16::MAX,
}

impl From<Hardfork> for SpecId {
    fn from(value: Hardfork) -> Self {
        match value {
            Hardfork::Byzantium => SpecId::BYZANTIUM,
            Hardfork::Constantinople => SpecId::CONSTANTINOPLE,
            Hardfork::Petersburg => SpecId::PETERSBURG,
            Hardfork::Istanbul => SpecId::ISTANBUL,
            Hardfork::MuirGlacier => SpecId::MUIR_GLACIER,
            Hardfork::Berlin => SpecId::BERLIN,
            Hardfork::London => SpecId::LONDON,
            Hardfork::ArrowGlacier => SpecId::ARROW_GLACIER,
            Hardfork::GrayGlacier => SpecId::GRAY_GLACIER,
            Hardfork::Merge => SpecId::MERGE,
            Hardfork::Shanghai => SpecId::SHANGHAI,
            Hardfork::Cancun => SpecId::CANCUN,
            Hardfork::Latest => SpecId::LATEST,
        }
    }
}

/// Error type that occurs when converting a string to a [`Hardfork`].
#[derive(Debug, thiserror::Error)]
pub enum ConversionError {
    /// Unknown hardfork name.
    #[error("Unknown hardfork name: {0}")]
    UnknownHardforkName(String),
}

impl TryFrom<&str> for Hardfork {
    type Error = ConversionError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            name::BYZANTIUM => Ok(Hardfork::Byzantium),
            name::CONSTANTINOPLE => Ok(Hardfork::Constantinople),
            name::PETERSBURG => Ok(Hardfork::Petersburg),
            name::ISTANBUL => Ok(Hardfork::Istanbul),
            name::MUIR_GLACIER => Ok(Hardfork::MuirGlacier),
            name::BERLIN => Ok(Hardfork::Berlin),
            name::LONDON => Ok(Hardfork::London),
            name::ARROW_GLACIER => Ok(Hardfork::ArrowGlacier),
            name::GRAY_GLACIER => Ok(Hardfork::GrayGlacier),
            name::MERGE => Ok(Hardfork::Merge),
            name::SHANGHAI => Ok(Hardfork::Shanghai),
            name::CANCUN => Ok(Hardfork::Cancun),
            name::LATEST => Ok(Hardfork::Latest),
            value => Err(ConversionError::UnknownHardforkName(value.to_string())),
        }
    }
}
