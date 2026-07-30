use edr_chain_spec::EvmSpecId;

/// Retrieves the miner reward for the provided hardfork.
pub fn miner_reward(spec_id: EvmSpecId) -> Option<u128> {
    match spec_id {
        EvmSpecId::FRONTIER
        | EvmSpecId::HOMESTEAD
        | EvmSpecId::TANGERINE
        | EvmSpecId::SPURIOUS_DRAGON => Some(5_000_000_000_000_000_000u128),
        EvmSpecId::BYZANTIUM => Some(3_000_000_000_000_000_000u128),
        EvmSpecId::PETERSBURG | EvmSpecId::ISTANBUL | EvmSpecId::BERLIN | EvmSpecId::LONDON => {
            Some(2_000_000_000_000_000_000u128)
        }
        _ => None,
    }
}
