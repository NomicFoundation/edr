// WARNING: This file is auto-generated. DO NOT EDIT MANUALLY.
// Any changes made to this file will be overwritten the next time it is
// generated. To make changes, update the generator script instead in
// `crates/tool/op_chain_config_generator/src/op_chain_config.rs`.

//! Auto-generated configs for OP stack chains

use edr_chain_config::ChainConfig;
use edr_primitives::HashMap;

use crate::Hardfork;

/// Chain configuration module for `arena_z`
pub mod arena_z;
/// Chain configuration module for `automata`
pub mod automata;
/// Chain configuration module for `base`
pub mod base;
/// Chain configuration module for `bob`
pub mod bob;
/// Chain configuration module for `boba`
pub mod boba;
/// Chain configuration module for `camp`
pub mod camp;
/// Chain configuration module for `celo`
pub mod celo;
/// Chain configuration module for `creator_chain_testnet`
pub mod creator_chain_testnet;
/// Chain configuration module for `cyber`
pub mod cyber;
/// Chain configuration module for `ethernity`
pub mod ethernity;
/// Chain configuration module for `fraxtal`
pub mod fraxtal;
/// Chain configuration module for `funki`
pub mod funki;
/// Chain configuration module for `hashkeychain`
pub mod hashkeychain;
/// Chain configuration module for `ink`
pub mod ink;
/// Chain configuration module for `lisk`
pub mod lisk;
/// Chain configuration module for `lyra`
pub mod lyra;
/// Chain configuration module for `metal`
pub mod metal;
/// Chain configuration module for `mint`
pub mod mint;
/// Chain configuration module for `mode`
pub mod mode;
/// Chain configuration module for `op`
pub mod op;
/// Chain configuration module for `orderly`
pub mod orderly;
/// Chain configuration module for `ozean`
pub mod ozean;
/// Chain configuration module for `pivotal`
pub mod pivotal;
/// Chain configuration module for `polynomial`
pub mod polynomial;
/// Chain configuration module for `race`
pub mod race;
/// Chain configuration module for `radius_testnet`
pub mod radius_testnet;
/// Chain configuration module for `redstone`
pub mod redstone;
/// Chain configuration module for `settlus_mainnet`
pub mod settlus_mainnet;
/// Chain configuration module for `settlus_sepolia`
pub mod settlus_sepolia;
/// Chain configuration module for `shape`
pub mod shape;
/// Chain configuration module for `silent_data_mainnet`
pub mod silent_data_mainnet;
/// Chain configuration module for `snax`
pub mod snax;
/// Chain configuration module for `soneium`
pub mod soneium;
/// Chain configuration module for `soneium_minato`
pub mod soneium_minato;
/// Chain configuration module for `sseed`
pub mod sseed;
/// Chain configuration module for `swan`
pub mod swan;
/// Chain configuration module for `swell`
pub mod swell;
/// Chain configuration module for `tbn`
pub mod tbn;
/// Chain configuration module for `unichain`
pub mod unichain;
/// Chain configuration module for `worldchain`
pub mod worldchain;
/// Chain configuration module for `xterio_eth`
pub mod xterio_eth;
/// Chain configuration module for `zora`
pub mod zora;

pub(super) fn chain_configs() -> HashMap<u64, ChainConfig<Hardfork>> {
    HashMap::from([
        (arena_z::MAINNET_CHAIN_ID, arena_z::mainnet_config()),
        (arena_z::SEPOLIA_CHAIN_ID, arena_z::sepolia_config()),
        (automata::MAINNET_CHAIN_ID, automata::mainnet_config()),
        (base::MAINNET_CHAIN_ID, base::mainnet_config()),
        (base::SEPOLIA_CHAIN_ID, base::sepolia_config()),
        (bob::MAINNET_CHAIN_ID, bob::mainnet_config()),
        (boba::MAINNET_CHAIN_ID, boba::mainnet_config()),
        (boba::SEPOLIA_CHAIN_ID, boba::sepolia_config()),
        (camp::SEPOLIA_CHAIN_ID, camp::sepolia_config()),
        (celo::MAINNET_CHAIN_ID, celo::mainnet_config()),
        (
            creator_chain_testnet::SEPOLIA_CHAIN_ID,
            creator_chain_testnet::sepolia_config(),
        ),
        (cyber::MAINNET_CHAIN_ID, cyber::mainnet_config()),
        (cyber::SEPOLIA_CHAIN_ID, cyber::sepolia_config()),
        (ethernity::MAINNET_CHAIN_ID, ethernity::mainnet_config()),
        (ethernity::SEPOLIA_CHAIN_ID, ethernity::sepolia_config()),
        (fraxtal::MAINNET_CHAIN_ID, fraxtal::mainnet_config()),
        (funki::MAINNET_CHAIN_ID, funki::mainnet_config()),
        (funki::SEPOLIA_CHAIN_ID, funki::sepolia_config()),
        (
            hashkeychain::MAINNET_CHAIN_ID,
            hashkeychain::mainnet_config(),
        ),
        (ink::MAINNET_CHAIN_ID, ink::mainnet_config()),
        (ink::SEPOLIA_CHAIN_ID, ink::sepolia_config()),
        (lisk::MAINNET_CHAIN_ID, lisk::mainnet_config()),
        (lisk::SEPOLIA_CHAIN_ID, lisk::sepolia_config()),
        (lyra::MAINNET_CHAIN_ID, lyra::mainnet_config()),
        (metal::MAINNET_CHAIN_ID, metal::mainnet_config()),
        (metal::SEPOLIA_CHAIN_ID, metal::sepolia_config()),
        (mint::MAINNET_CHAIN_ID, mint::mainnet_config()),
        (mode::MAINNET_CHAIN_ID, mode::mainnet_config()),
        (mode::SEPOLIA_CHAIN_ID, mode::sepolia_config()),
        (op::MAINNET_CHAIN_ID, op::mainnet_config()),
        (op::SEPOLIA_CHAIN_ID, op::sepolia_config()),
        (orderly::MAINNET_CHAIN_ID, orderly::mainnet_config()),
        (ozean::SEPOLIA_CHAIN_ID, ozean::sepolia_config()),
        (pivotal::SEPOLIA_CHAIN_ID, pivotal::sepolia_config()),
        (polynomial::MAINNET_CHAIN_ID, polynomial::mainnet_config()),
        (race::MAINNET_CHAIN_ID, race::mainnet_config()),
        (race::SEPOLIA_CHAIN_ID, race::sepolia_config()),
        (
            radius_testnet::SEPOLIA_CHAIN_ID,
            radius_testnet::sepolia_config(),
        ),
        (redstone::MAINNET_CHAIN_ID, redstone::mainnet_config()),
        (
            settlus_mainnet::MAINNET_CHAIN_ID,
            settlus_mainnet::mainnet_config(),
        ),
        (
            settlus_sepolia::SEPOLIA_CHAIN_ID,
            settlus_sepolia::sepolia_config(),
        ),
        (shape::MAINNET_CHAIN_ID, shape::mainnet_config()),
        (shape::SEPOLIA_CHAIN_ID, shape::sepolia_config()),
        (
            silent_data_mainnet::MAINNET_CHAIN_ID,
            silent_data_mainnet::mainnet_config(),
        ),
        (snax::MAINNET_CHAIN_ID, snax::mainnet_config()),
        (soneium::MAINNET_CHAIN_ID, soneium::mainnet_config()),
        (
            soneium_minato::SEPOLIA_CHAIN_ID,
            soneium_minato::sepolia_config(),
        ),
        (sseed::MAINNET_CHAIN_ID, sseed::mainnet_config()),
        (swan::MAINNET_CHAIN_ID, swan::mainnet_config()),
        (swell::MAINNET_CHAIN_ID, swell::mainnet_config()),
        (tbn::MAINNET_CHAIN_ID, tbn::mainnet_config()),
        (tbn::SEPOLIA_CHAIN_ID, tbn::sepolia_config()),
        (unichain::MAINNET_CHAIN_ID, unichain::mainnet_config()),
        (unichain::SEPOLIA_CHAIN_ID, unichain::sepolia_config()),
        (worldchain::MAINNET_CHAIN_ID, worldchain::mainnet_config()),
        (worldchain::SEPOLIA_CHAIN_ID, worldchain::sepolia_config()),
        (xterio_eth::MAINNET_CHAIN_ID, xterio_eth::mainnet_config()),
        (zora::MAINNET_CHAIN_ID, zora::mainnet_config()),
        (zora::SEPOLIA_CHAIN_ID, zora::sepolia_config()),
    ])
}
