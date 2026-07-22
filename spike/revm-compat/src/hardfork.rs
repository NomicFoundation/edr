//! Spike finding: EDR needs a hardfork newtype for OP.
//!
//! `edr_chain_spec::HardforkChainSpec` requires `Hardfork: Into<EvmSpecId>`,
//! where `EvmSpecId` becomes revm@41's `SpecId` after the upgrade. op-revm@20
//! only implements `From<OpSpecId>` for revm@38's `SpecId`, and the orphan
//! rule forbids EDR from implementing a foreign trait (`From`) between two
//! foreign types. So `edr_op`'s `Hardfork` type alias must become a newtype.

use op_revm::OpSpecId;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OpHardfork(pub OpSpecId);

/// Mirrors op-revm@20's `OpSpecId::into_eth_spec`, targeting revm@41's
/// `SpecId`. Must be kept in sync with op-revm on every bump.
impl From<OpHardfork> for revm41::primitives::hardfork::SpecId {
    fn from(hardfork: OpHardfork) -> Self {
        match hardfork.0 {
            OpSpecId::BEDROCK | OpSpecId::REGOLITH => Self::MERGE,
            OpSpecId::CANYON => Self::SHANGHAI,
            OpSpecId::ECOTONE | OpSpecId::FJORD | OpSpecId::GRANITE | OpSpecId::HOLOCENE => {
                Self::CANCUN
            }
            OpSpecId::ISTHMUS | OpSpecId::JOVIAN | OpSpecId::INTEROP => Self::PRAGUE,
            OpSpecId::KARST => Self::OSAKA,
        }
    }
}
