use napi_derive::napi;

/// Identifier for the Ethereum spec.
#[napi]
pub enum SpecId {
    /// Frontier
    Frontier = 0,
    /// Frontier Thawing
    FrontierThawing = 1,
    /// Homestead
    Homestead = 2,
    /// DAO Fork
    DaoFork = 3,
    /// Tangerine
    Tangerine = 4,
    /// Spurious Dragon
    SpuriousDragon = 5,
    /// Byzantium
    Byzantium = 6,
    /// Constantinople
    Constantinople = 7,
    /// Petersburg
    Petersburg = 8,
    /// Istanbul
    Istanbul = 9,
    /// Muir Glacier
    MuirGlacier = 10,
    /// Berlin
    Berlin = 11,
    /// London
    London = 12,
    /// Arrow Glacier
    ArrowGlacier = 13,
    /// Gray Glacier
    GrayGlacier = 14,
    /// Merge
    Merge = 15,
    /// Shanghai
    Shanghai = 16,
    /// Cancun
    Cancun = 17,
    /// Latest
    Latest = 18,
}

impl From<SpecId> for edr_eth::SpecId {
    fn from(value: SpecId) -> Self {
        match value {
            SpecId::Frontier => edr_eth::SpecId::FRONTIER,
            SpecId::FrontierThawing => edr_eth::SpecId::FRONTIER_THAWING,
            SpecId::Homestead => edr_eth::SpecId::HOMESTEAD,
            SpecId::DaoFork => edr_eth::SpecId::DAO_FORK,
            SpecId::Tangerine => edr_eth::SpecId::TANGERINE,
            SpecId::SpuriousDragon => edr_eth::SpecId::SPURIOUS_DRAGON,
            SpecId::Byzantium => edr_eth::SpecId::BYZANTIUM,
            SpecId::Constantinople => edr_eth::SpecId::CONSTANTINOPLE,
            SpecId::Petersburg => edr_eth::SpecId::PETERSBURG,
            SpecId::Istanbul => edr_eth::SpecId::ISTANBUL,
            SpecId::MuirGlacier => edr_eth::SpecId::MUIR_GLACIER,
            SpecId::Berlin => edr_eth::SpecId::BERLIN,
            SpecId::London => edr_eth::SpecId::LONDON,
            SpecId::ArrowGlacier => edr_eth::SpecId::ARROW_GLACIER,
            SpecId::GrayGlacier => edr_eth::SpecId::GRAY_GLACIER,
            SpecId::Merge => edr_eth::SpecId::MERGE,
            SpecId::Shanghai => edr_eth::SpecId::SHANGHAI,
            SpecId::Cancun => edr_eth::SpecId::CANCUN,
            SpecId::Latest => edr_eth::SpecId::LATEST,
        }
    }
}
