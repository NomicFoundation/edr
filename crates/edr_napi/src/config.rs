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

impl From<SpecId> for edr_evm::EthSpecId {
    fn from(value: SpecId) -> Self {
        match value {
            SpecId::Frontier => edr_evm::EthSpecId::FRONTIER,
            SpecId::FrontierThawing => edr_evm::EthSpecId::FRONTIER_THAWING,
            SpecId::Homestead => edr_evm::EthSpecId::HOMESTEAD,
            SpecId::DaoFork => edr_evm::EthSpecId::DAO_FORK,
            SpecId::Tangerine => edr_evm::EthSpecId::TANGERINE,
            SpecId::SpuriousDragon => edr_evm::EthSpecId::SPURIOUS_DRAGON,
            SpecId::Byzantium => edr_evm::EthSpecId::BYZANTIUM,
            SpecId::Constantinople => edr_evm::EthSpecId::CONSTANTINOPLE,
            SpecId::Petersburg => edr_evm::EthSpecId::PETERSBURG,
            SpecId::Istanbul => edr_evm::EthSpecId::ISTANBUL,
            SpecId::MuirGlacier => edr_evm::EthSpecId::MUIR_GLACIER,
            SpecId::Berlin => edr_evm::EthSpecId::BERLIN,
            SpecId::London => edr_evm::EthSpecId::LONDON,
            SpecId::ArrowGlacier => edr_evm::EthSpecId::ARROW_GLACIER,
            SpecId::GrayGlacier => edr_evm::EthSpecId::GRAY_GLACIER,
            SpecId::Merge => edr_evm::EthSpecId::MERGE,
            SpecId::Shanghai => edr_evm::EthSpecId::SHANGHAI,
            SpecId::Cancun => edr_evm::EthSpecId::CANCUN,
            SpecId::Latest => edr_evm::EthSpecId::LATEST,
        }
    }
}
