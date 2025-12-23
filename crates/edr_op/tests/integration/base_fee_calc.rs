#![cfg(feature = "test-remote")]

use edr_op::{OpChainSpec, block::decode_base_params};
use edr_provider::test_utils::header_overrides;
use edr_test_block_replay::assert_replay_header;
use alloy_eips::calc_next_block_base_fee;

use crate::integration::base;

macro_rules! impl_test_base_fee_calc{
    ($($net:ident: $url:expr => [
        $($block_number:literal,)+
    ],)+) => {
        $(
            $(
                paste::item! {
                    #[serial_test::serial]
                    #[tokio::test(flavor = "multi_thread")]
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
            let alloy_calculation = calc_next_block_base_fee(parent_header.gas_used, parent_header.gas_limit, parent_base_fee_per_gas, base_fee_params);
            assert_eq!(local_header.base_fee, Some(u128::from(alloy_calculation)));
            assert_eq!(remote_header.base_fee_per_gas, local_header.base_fee);
            Ok(())
        },
    )
    .await
}

impl_test_base_fee_calc! {
    base_mainnet: base::mainnet_url() => [
        // blocks from 2025-12-18
        39628091,
        39628092,
        39628093,
        39628094,
        39628095,
        39628096,
        39628097,
        39628098,
        39628099,
        39628100,
        39628101,
        // blocks from 2025-12-23
        39842838,
        39842839,
        39842840,
        39842877,
        39842878,
        39842879,
    ],
}
