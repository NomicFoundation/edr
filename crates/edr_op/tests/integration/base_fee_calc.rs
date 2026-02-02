#![cfg(feature = "test-remote")]

use alloy_eips::calc_next_block_base_fee;
use edr_op::{block::decode_base_params, OpChainSpec};
use edr_provider::test_utils::header_overrides;
use edr_test_block_replay::assert_replay_header;
use edr_test_utils::env::json_rpc_url_provider;

macro_rules! impl_test_base_fee_calc{
    ($($net:ident: $url:expr => [
        $($block_number:literal,)+
    ],)+) => {
        $(
            $(
                paste::item! {
                    #[serial_test::serial]
                    #[tokio::test(flavor = "multi_thread")]
                    #[cfg(feature = "test-remote")]
                    async fn [<test_dynamic_base_fee_ $net _ $block_number>]() -> anyhow::Result<()> {
                        let runtime = tokio::runtime::Handle::current();
                        let url = $url;

                        assert_base_fee_per_gas(
                            runtime,
                            url,
                            $block_number,
                        )
                        .await
                    }
                }
            )+
        )+
    };
}

async fn assert_base_fee_per_gas(
    runtime: tokio::runtime::Handle,
    url: String,
    block_number: u64,
) -> anyhow::Result<()> {
    assert_replay_header::<OpChainSpec>(
        runtime,
        url,
        block_number,
        header_overrides,
        |remote_header, local_header, parent_header| {
            let base_fee_params = decode_base_params(&parent_header.extra_data);
            let parent_base_fee_per_gas = parent_header.base_fee_per_gas.unwrap() as u64;
            let alloy_calculation = calc_next_block_base_fee(
                parent_header.gas_used,
                parent_header.gas_limit,
                parent_base_fee_per_gas,
                base_fee_params,
            );
            assert_eq!(local_header.base_fee, Some(u128::from(alloy_calculation)));
            assert_eq!(remote_header.base_fee_per_gas, local_header.base_fee);
            Ok(())
        },
    )
    .await
}

impl_test_base_fee_calc! {
    base_mainnet: json_rpc_url_provider::base_mainnet() => [
        // blocks from 2025-12-18 - BaseFeeParams(50, 5)
        39628091, // parent gas_used below target
        39628092, // parent gas_used over target - FAILS
        39628093, // parent gas_used over target - FAILS
        39628094, // parent gas_used over target - FAILS
        39628095, // parent gas_used over target - FAILS
        39628096, // parent gas_used over target - FAILS
        39628097, // parent gas_used over target - FAILS
        39628098, // parent gas_used over target
        39628099, // parent gas_used below target
        39628100, // parent gas_used over target
        39628101, // parent gas_used below target
        // blocks from 2025-12-23 - BaseFeeParams(50, 6)
        39842838, // parent gas_used below target
        39842839, // parent gas_used below target
        39842840, // parent gas_used above target
        39842841, // parent gas_used below target
        39842877, // parent gas_used below target
        39842878, // parent gas_used below target
        39842879, // parent gas_used above target
        39842880, // parent gas_used below target
    ],
}
