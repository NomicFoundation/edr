use alloy_eips::eip1559::INITIAL_BASE_FEE;
use edr_block_header::{
    calculate_next_base_fee_per_gas, BlockConfig, BlockHeader, HeaderOverrides, PartialHeader,
};
use edr_chain_l1::{
    chains::{l1_chain_config, L1_MAINNET_CHAIN_ID},
    L1_MIN_ETHASH_DIFFICULTY,
};
use edr_eip1559::{BaseFeeParams, ConstantBaseFeeParams};

const DEFAULT_INITIAL_BASE_FEE: u128 = INITIAL_BASE_FEE as u128;

#[test]
fn test_partial_header_uses_base_fee_override() {
    let ommers = vec![];
    let configured_base_fee = 2_000_000_000;
    let overrides = HeaderOverrides {
        base_fee: Some(configured_base_fee),
        ..HeaderOverrides::default()
    };

    let chain_config =
        l1_chain_config(L1_MAINNET_CHAIN_ID).expect("L1 Mainnet config should exist");

    let block_config = BlockConfig {
        base_fee_params: chain_config.base_fee_params.clone(),
        hardfork: edr_chain_l1::Hardfork::LONDON,
        min_ethash_difficulty: L1_MIN_ETHASH_DIFFICULTY,
        scheduled_blob_params: None,
    };
    let partial_header =
        PartialHeader::new::<edr_chain_l1::Hardfork>(&block_config, overrides, None, &ommers, None);

    assert_eq!(partial_header.base_fee, Some(configured_base_fee));
}

#[test]
fn test_partial_header_base_fee_override_takes_precedence_over_base_fee_params_override() {
    let ommers = vec![];
    let configured_base_fee = 2_000_000_000;
    let base_fee_params = BaseFeeParams::Constant(ConstantBaseFeeParams {
        max_change_denominator: 50,
        elasticity_multiplier: 2,
    });
    let overrides = HeaderOverrides {
        base_fee: Some(configured_base_fee),
        base_fee_params: Some(base_fee_params),
        ..HeaderOverrides::default()
    };

    let chain_config =
        l1_chain_config(L1_MAINNET_CHAIN_ID).expect("L1 Mainnet config should exist");

    let block_config = BlockConfig {
        base_fee_params: chain_config.base_fee_params.clone(),
        hardfork: edr_chain_l1::Hardfork::LONDON,
        min_ethash_difficulty: L1_MIN_ETHASH_DIFFICULTY,
        scheduled_blob_params: None,
    };
    let partial_header =
        PartialHeader::new::<edr_chain_l1::Hardfork>(&block_config, overrides, None, &ommers, None);

    assert_eq!(partial_header.base_fee, Some(configured_base_fee));
}

#[test]
fn test_partial_header_ignores_base_fee_params_if_before_london() {
    let ommers = vec![];
    let overrides = HeaderOverrides {
        base_fee_params: Some(BaseFeeParams::Constant(ConstantBaseFeeParams {
            max_change_denominator: 50,
            elasticity_multiplier: 2,
        })),
        ..HeaderOverrides::default()
    };

    let chain_config =
        l1_chain_config(L1_MAINNET_CHAIN_ID).expect("L1 Mainnet config should exist");

    let block_config = BlockConfig {
        base_fee_params: chain_config.base_fee_params.clone(),
        hardfork: edr_chain_l1::Hardfork::BERLIN,
        min_ethash_difficulty: L1_MIN_ETHASH_DIFFICULTY,
        scheduled_blob_params: None,
    };
    let partial_header =
        PartialHeader::new::<edr_chain_l1::Hardfork>(&block_config, overrides, None, &ommers, None);

    assert_eq!(partial_header.base_fee, None);
}

#[test]
fn test_partial_header_defaults_base_fee_if_no_override_after_london() {
    let ommers = vec![];
    let overrides = HeaderOverrides::default();

    let chain_config =
        l1_chain_config(L1_MAINNET_CHAIN_ID).expect("L1 Mainnet config should exist");

    let block_config = BlockConfig {
        base_fee_params: chain_config.base_fee_params.clone(),
        hardfork: edr_chain_l1::Hardfork::LONDON,
        min_ethash_difficulty: L1_MIN_ETHASH_DIFFICULTY,
        scheduled_blob_params: None,
    };

    let partial_header =
        PartialHeader::new::<edr_chain_l1::Hardfork>(&block_config, overrides, None, &ommers, None);

    assert_eq!(partial_header.base_fee, Some(DEFAULT_INITIAL_BASE_FEE));
}

#[test]
fn test_partial_header_defaults_base_fee_if_no_parent_after_london() {
    let ommers = vec![];
    let overrides = HeaderOverrides {
        base_fee_params: Some(BaseFeeParams::Constant(ConstantBaseFeeParams {
            max_change_denominator: 50,
            elasticity_multiplier: 2,
        })),
        ..HeaderOverrides::default()
    };

    let chain_config =
        l1_chain_config(L1_MAINNET_CHAIN_ID).expect("L1 Mainnet config should exist");

    let block_config = BlockConfig {
        base_fee_params: chain_config.base_fee_params.clone(),
        hardfork: edr_chain_l1::Hardfork::LONDON,
        min_ethash_difficulty: L1_MIN_ETHASH_DIFFICULTY,
        scheduled_blob_params: None,
    };
    let partial_header =
        PartialHeader::new::<edr_chain_l1::Hardfork>(&block_config, overrides, None, &ommers, None);

    assert_eq!(partial_header.base_fee, Some(DEFAULT_INITIAL_BASE_FEE));
}

#[test]
fn test_partial_header_uses_override_with_parent_after_london() {
    let ommers = vec![];
    let base_fee_params = BaseFeeParams::Constant(ConstantBaseFeeParams {
        max_change_denominator: 50,
        elasticity_multiplier: 2,
    });
    let overrides = HeaderOverrides {
        base_fee_params: Some(base_fee_params.clone()),
        ..HeaderOverrides::default()
    };

    let chain_config =
        l1_chain_config(L1_MAINNET_CHAIN_ID).expect("L1 Mainnet config should exist");

    let parent_header = BlockHeader {
        base_fee_per_gas: Some(DEFAULT_INITIAL_BASE_FEE),
        gas_limit: 0xffffffffffffff,
        gas_used: 200,
        ..BlockHeader::default()
    };
    let block_config = BlockConfig {
        base_fee_params: chain_config.base_fee_params.clone(),
        hardfork: edr_chain_l1::Hardfork::LONDON,
        min_ethash_difficulty: L1_MIN_ETHASH_DIFFICULTY,
        scheduled_blob_params: None,
    };
    let partial_header = PartialHeader::new::<edr_chain_l1::Hardfork>(
        &block_config,
        overrides,
        Some(&parent_header),
        &ommers,
        None,
    );

    assert_eq!(
        partial_header.base_fee,
        Some(calculate_next_base_fee_per_gas(
            &parent_header,
            &base_fee_params,
            &edr_chain_l1::Hardfork::LONDON
        ))
    );
}
