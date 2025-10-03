// WARNING: This file is auto-generated. DO NOT EDIT MANUALLY.
// Any changes made to this file will be overwritten the next time it is
// generated. To make changes, update the generator script instead in
// `tools/src/op_chain_config.rs`.

use edr_evm::hardfork::ChainConfig;
use edr_primitives::HashMap;

use crate::Hardfork;

/// `arena_z` chain configuration module;
pub mod arena_z;
/// `automata` chain configuration module;
pub mod automata;
/// `base` chain configuration module;
pub mod base;
/// `bob` chain configuration module;
pub mod bob;
/// `boba` chain configuration module;
pub mod boba;
/// `camp` chain configuration module;
pub mod camp;
/// `celo` chain configuration module;
pub mod celo;
/// `creator_chain_testnet` chain configuration module;
pub mod creator_chain_testnet;
/// `cyber` chain configuration module;
pub mod cyber;
/// `ethernity` chain configuration module;
pub mod ethernity;
/// `fraxtal` chain configuration module;
pub mod fraxtal;
/// `funki` chain configuration module;
pub mod funki;
/// `hashkeychain` chain configuration module;
pub mod hashkeychain;
/// `ink` chain configuration module;
pub mod ink;
/// `lisk` chain configuration module;
pub mod lisk;
/// `lyra` chain configuration module;
pub mod lyra;
/// `metal` chain configuration module;
pub mod metal;
/// `mint` chain configuration module;
pub mod mint;
/// `mode` chain configuration module;
pub mod mode;
/// `op` chain configuration module;
pub mod op;
/// `orderly` chain configuration module;
pub mod orderly;
/// `ozean` chain configuration module;
pub mod ozean;
/// `pivotal` chain configuration module;
pub mod pivotal;
/// `polynomial` chain configuration module;
pub mod polynomial;
/// `race` chain configuration module;
pub mod race;
/// `radius_testnet` chain configuration module;
pub mod radius_testnet;
/// `redstone` chain configuration module;
pub mod redstone;
/// `settlus_mainnet` chain configuration module;
pub mod settlus_mainnet;
/// `settlus_sepolia` chain configuration module;
pub mod settlus_sepolia;
/// `shape` chain configuration module;
pub mod shape;
/// `silent_data_mainnet` chain configuration module;
pub mod silent_data_mainnet;
/// `snax` chain configuration module;
pub mod snax;
/// `soneium` chain configuration module;
pub mod soneium;
/// `soneium_minato` chain configuration module;
pub mod soneium_minato;
/// `sseed` chain configuration module;
pub mod sseed;
/// `swan` chain configuration module;
pub mod swan;
/// `swell` chain configuration module;
pub mod swell;
/// `tbn` chain configuration module;
pub mod tbn;
/// `unichain` chain configuration module;
pub mod unichain;
/// `worldchain` chain configuration module;
pub mod worldchain;
/// `xterio_eth` chain configuration module;
pub mod xterio_eth;
/// `zora` chain configuration module;
pub mod zora;

pub(crate) fn chain_configs() -> HashMap<u64, ChainConfig<Hardfork>> {
    let mut hardforks = HashMap::new();

    hardforks.insert(arena_z::MAINNET_CHAIN_ID, arena_z::mainnet_config());
    hardforks.insert(arena_z::SEPOLIA_CHAIN_ID, arena_z::sepolia_config());
    hardforks.insert(automata::MAINNET_CHAIN_ID, automata::mainnet_config());
    hardforks.insert(base::MAINNET_CHAIN_ID, base::mainnet_config());
    hardforks.insert(base::SEPOLIA_CHAIN_ID, base::sepolia_config());
    hardforks.insert(bob::MAINNET_CHAIN_ID, bob::mainnet_config());
    hardforks.insert(boba::MAINNET_CHAIN_ID, boba::mainnet_config());
    hardforks.insert(boba::SEPOLIA_CHAIN_ID, boba::sepolia_config());
    hardforks.insert(camp::SEPOLIA_CHAIN_ID, camp::sepolia_config());
    hardforks.insert(celo::MAINNET_CHAIN_ID, celo::mainnet_config());
    hardforks.insert(
        creator_chain_testnet::SEPOLIA_CHAIN_ID,
        creator_chain_testnet::sepolia_config(),
    );
    hardforks.insert(cyber::MAINNET_CHAIN_ID, cyber::mainnet_config());
    hardforks.insert(cyber::SEPOLIA_CHAIN_ID, cyber::sepolia_config());
    hardforks.insert(ethernity::MAINNET_CHAIN_ID, ethernity::mainnet_config());
    hardforks.insert(ethernity::SEPOLIA_CHAIN_ID, ethernity::sepolia_config());
    hardforks.insert(fraxtal::MAINNET_CHAIN_ID, fraxtal::mainnet_config());
    hardforks.insert(funki::MAINNET_CHAIN_ID, funki::mainnet_config());
    hardforks.insert(funki::SEPOLIA_CHAIN_ID, funki::sepolia_config());
    hardforks.insert(
        hashkeychain::MAINNET_CHAIN_ID,
        hashkeychain::mainnet_config(),
    );
    hardforks.insert(ink::MAINNET_CHAIN_ID, ink::mainnet_config());
    hardforks.insert(ink::SEPOLIA_CHAIN_ID, ink::sepolia_config());
    hardforks.insert(lisk::MAINNET_CHAIN_ID, lisk::mainnet_config());
    hardforks.insert(lisk::SEPOLIA_CHAIN_ID, lisk::sepolia_config());
    hardforks.insert(lyra::MAINNET_CHAIN_ID, lyra::mainnet_config());
    hardforks.insert(metal::MAINNET_CHAIN_ID, metal::mainnet_config());
    hardforks.insert(metal::SEPOLIA_CHAIN_ID, metal::sepolia_config());
    hardforks.insert(mint::MAINNET_CHAIN_ID, mint::mainnet_config());
    hardforks.insert(mode::MAINNET_CHAIN_ID, mode::mainnet_config());
    hardforks.insert(mode::SEPOLIA_CHAIN_ID, mode::sepolia_config());
    hardforks.insert(op::MAINNET_CHAIN_ID, op::mainnet_config());
    hardforks.insert(op::SEPOLIA_CHAIN_ID, op::sepolia_config());
    hardforks.insert(orderly::MAINNET_CHAIN_ID, orderly::mainnet_config());
    hardforks.insert(ozean::SEPOLIA_CHAIN_ID, ozean::sepolia_config());
    hardforks.insert(pivotal::SEPOLIA_CHAIN_ID, pivotal::sepolia_config());
    hardforks.insert(polynomial::MAINNET_CHAIN_ID, polynomial::mainnet_config());
    hardforks.insert(race::MAINNET_CHAIN_ID, race::mainnet_config());
    hardforks.insert(race::SEPOLIA_CHAIN_ID, race::sepolia_config());
    hardforks.insert(
        radius_testnet::SEPOLIA_CHAIN_ID,
        radius_testnet::sepolia_config(),
    );
    hardforks.insert(redstone::MAINNET_CHAIN_ID, redstone::mainnet_config());
    hardforks.insert(
        settlus_mainnet::MAINNET_CHAIN_ID,
        settlus_mainnet::mainnet_config(),
    );
    hardforks.insert(
        settlus_sepolia::SEPOLIA_CHAIN_ID,
        settlus_sepolia::sepolia_config(),
    );
    hardforks.insert(shape::MAINNET_CHAIN_ID, shape::mainnet_config());
    hardforks.insert(shape::SEPOLIA_CHAIN_ID, shape::sepolia_config());
    hardforks.insert(
        silent_data_mainnet::MAINNET_CHAIN_ID,
        silent_data_mainnet::mainnet_config(),
    );
    hardforks.insert(snax::MAINNET_CHAIN_ID, snax::mainnet_config());
    hardforks.insert(soneium::MAINNET_CHAIN_ID, soneium::mainnet_config());
    hardforks.insert(
        soneium_minato::SEPOLIA_CHAIN_ID,
        soneium_minato::sepolia_config(),
    );
    hardforks.insert(sseed::MAINNET_CHAIN_ID, sseed::mainnet_config());
    hardforks.insert(swan::MAINNET_CHAIN_ID, swan::mainnet_config());
    hardforks.insert(swell::MAINNET_CHAIN_ID, swell::mainnet_config());
    hardforks.insert(tbn::MAINNET_CHAIN_ID, tbn::mainnet_config());
    hardforks.insert(tbn::SEPOLIA_CHAIN_ID, tbn::sepolia_config());
    hardforks.insert(unichain::MAINNET_CHAIN_ID, unichain::mainnet_config());
    hardforks.insert(unichain::SEPOLIA_CHAIN_ID, unichain::sepolia_config());
    hardforks.insert(worldchain::MAINNET_CHAIN_ID, worldchain::mainnet_config());
    hardforks.insert(worldchain::SEPOLIA_CHAIN_ID, worldchain::sepolia_config());
    hardforks.insert(xterio_eth::MAINNET_CHAIN_ID, xterio_eth::mainnet_config());
    hardforks.insert(zora::MAINNET_CHAIN_ID, zora::mainnet_config());
    hardforks.insert(zora::SEPOLIA_CHAIN_ID, zora::sepolia_config());

    hardforks
}
