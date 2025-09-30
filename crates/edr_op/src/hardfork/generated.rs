// WARNING: This file is auto-generated. DO NOT EDIT MANUALLY.
// Any changes made to this file will be overwritten the next time it is
// generated. To make changes, update the generator script instead
// (tools/op_chain_config.rs).

use std::{collections::HashMap, sync::OnceLock};

use edr_evm::hardfork::ChainConfig;

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

fn chain_configs() -> &'static HashMap<u64, &'static ChainConfig<Hardfork>> {
    static CONFIGS: OnceLock<HashMap<u64, &'static ChainConfig<Hardfork>>> = OnceLock::new();

    CONFIGS.get_or_init(|| {
        let mut hardforks = HashMap::new();

        hardforks.insert(arena_z::MAINNET_CHAIN_ID, &*arena_z::MAINNET_CONFIG);

        hardforks.insert(arena_z::SEPOLIA_CHAIN_ID, &*arena_z::SEPOLIA_CONFIG);

        hardforks.insert(automata::MAINNET_CHAIN_ID, &*automata::MAINNET_CONFIG);

        hardforks.insert(base::MAINNET_CHAIN_ID, &*base::MAINNET_CONFIG);

        hardforks.insert(base::SEPOLIA_CHAIN_ID, &*base::SEPOLIA_CONFIG);

        hardforks.insert(bob::MAINNET_CHAIN_ID, &*bob::MAINNET_CONFIG);

        hardforks.insert(boba::MAINNET_CHAIN_ID, &*boba::MAINNET_CONFIG);

        hardforks.insert(boba::SEPOLIA_CHAIN_ID, &*boba::SEPOLIA_CONFIG);

        hardforks.insert(camp::SEPOLIA_CHAIN_ID, &*camp::SEPOLIA_CONFIG);

        hardforks.insert(celo::MAINNET_CHAIN_ID, &*celo::MAINNET_CONFIG);

        hardforks.insert(
            creator_chain_testnet::SEPOLIA_CHAIN_ID,
            &*creator_chain_testnet::SEPOLIA_CONFIG,
        );

        hardforks.insert(cyber::MAINNET_CHAIN_ID, &*cyber::MAINNET_CONFIG);

        hardforks.insert(cyber::SEPOLIA_CHAIN_ID, &*cyber::SEPOLIA_CONFIG);

        hardforks.insert(ethernity::MAINNET_CHAIN_ID, &*ethernity::MAINNET_CONFIG);

        hardforks.insert(ethernity::SEPOLIA_CHAIN_ID, &*ethernity::SEPOLIA_CONFIG);

        hardforks.insert(fraxtal::MAINNET_CHAIN_ID, &*fraxtal::MAINNET_CONFIG);

        hardforks.insert(funki::MAINNET_CHAIN_ID, &*funki::MAINNET_CONFIG);

        hardforks.insert(funki::SEPOLIA_CHAIN_ID, &*funki::SEPOLIA_CONFIG);

        hardforks.insert(
            hashkeychain::MAINNET_CHAIN_ID,
            &*hashkeychain::MAINNET_CONFIG,
        );

        hardforks.insert(ink::MAINNET_CHAIN_ID, &*ink::MAINNET_CONFIG);

        hardforks.insert(ink::SEPOLIA_CHAIN_ID, &*ink::SEPOLIA_CONFIG);

        hardforks.insert(lisk::MAINNET_CHAIN_ID, &*lisk::MAINNET_CONFIG);

        hardforks.insert(lisk::SEPOLIA_CHAIN_ID, &*lisk::SEPOLIA_CONFIG);

        hardforks.insert(lyra::MAINNET_CHAIN_ID, &*lyra::MAINNET_CONFIG);

        hardforks.insert(metal::MAINNET_CHAIN_ID, &*metal::MAINNET_CONFIG);

        hardforks.insert(metal::SEPOLIA_CHAIN_ID, &*metal::SEPOLIA_CONFIG);

        hardforks.insert(mint::MAINNET_CHAIN_ID, &*mint::MAINNET_CONFIG);

        hardforks.insert(mode::MAINNET_CHAIN_ID, &*mode::MAINNET_CONFIG);

        hardforks.insert(mode::SEPOLIA_CHAIN_ID, &*mode::SEPOLIA_CONFIG);

        hardforks.insert(op::MAINNET_CHAIN_ID, &*op::MAINNET_CONFIG);

        hardforks.insert(op::SEPOLIA_CHAIN_ID, &*op::SEPOLIA_CONFIG);

        hardforks.insert(orderly::MAINNET_CHAIN_ID, &*orderly::MAINNET_CONFIG);

        hardforks.insert(ozean::SEPOLIA_CHAIN_ID, &*ozean::SEPOLIA_CONFIG);

        hardforks.insert(pivotal::SEPOLIA_CHAIN_ID, &*pivotal::SEPOLIA_CONFIG);

        hardforks.insert(polynomial::MAINNET_CHAIN_ID, &*polynomial::MAINNET_CONFIG);

        hardforks.insert(race::MAINNET_CHAIN_ID, &*race::MAINNET_CONFIG);

        hardforks.insert(race::SEPOLIA_CHAIN_ID, &*race::SEPOLIA_CONFIG);

        hardforks.insert(
            radius_testnet::SEPOLIA_CHAIN_ID,
            &*radius_testnet::SEPOLIA_CONFIG,
        );

        hardforks.insert(redstone::MAINNET_CHAIN_ID, &*redstone::MAINNET_CONFIG);

        hardforks.insert(
            settlus_mainnet::MAINNET_CHAIN_ID,
            &*settlus_mainnet::MAINNET_CONFIG,
        );

        hardforks.insert(
            settlus_sepolia::SEPOLIA_CHAIN_ID,
            &*settlus_sepolia::SEPOLIA_CONFIG,
        );

        hardforks.insert(shape::MAINNET_CHAIN_ID, &*shape::MAINNET_CONFIG);

        hardforks.insert(shape::SEPOLIA_CHAIN_ID, &*shape::SEPOLIA_CONFIG);

        hardforks.insert(
            silent_data_mainnet::MAINNET_CHAIN_ID,
            &*silent_data_mainnet::MAINNET_CONFIG,
        );

        hardforks.insert(snax::MAINNET_CHAIN_ID, &*snax::MAINNET_CONFIG);

        hardforks.insert(soneium::MAINNET_CHAIN_ID, &*soneium::MAINNET_CONFIG);

        hardforks.insert(
            soneium_minato::SEPOLIA_CHAIN_ID,
            &*soneium_minato::SEPOLIA_CONFIG,
        );

        hardforks.insert(sseed::MAINNET_CHAIN_ID, &*sseed::MAINNET_CONFIG);

        hardforks.insert(swan::MAINNET_CHAIN_ID, &*swan::MAINNET_CONFIG);

        hardforks.insert(swell::MAINNET_CHAIN_ID, &*swell::MAINNET_CONFIG);

        hardforks.insert(tbn::MAINNET_CHAIN_ID, &*tbn::MAINNET_CONFIG);

        hardforks.insert(tbn::SEPOLIA_CHAIN_ID, &*tbn::SEPOLIA_CONFIG);

        hardforks.insert(unichain::MAINNET_CHAIN_ID, &*unichain::MAINNET_CONFIG);

        hardforks.insert(unichain::SEPOLIA_CHAIN_ID, &*unichain::SEPOLIA_CONFIG);

        hardforks.insert(worldchain::MAINNET_CHAIN_ID, &*worldchain::MAINNET_CONFIG);

        hardforks.insert(worldchain::SEPOLIA_CHAIN_ID, &*worldchain::SEPOLIA_CONFIG);

        hardforks.insert(xterio_eth::MAINNET_CHAIN_ID, &*xterio_eth::MAINNET_CONFIG);

        hardforks.insert(zora::MAINNET_CHAIN_ID, &*zora::MAINNET_CONFIG);

        hardforks.insert(zora::SEPOLIA_CHAIN_ID, &*zora::SEPOLIA_CONFIG);

        hardforks
    })
}
