#![cfg(feature = "test-remote")]

use edr_block_header::{BlockHeader, PartialHeader};
use edr_op::OpChainSpec;
use edr_provider::test_utils::header_overrides;
use edr_test_block_replay::assert_replay_header;
use edr_test_utils::env::json_rpc_url_provider;

macro_rules! impl_test_dynamic_base_fee_params{
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

                        assert_base_fee_activation(
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

async fn assert_base_fee_activation(
    runtime: tokio::runtime::Handle,
    url: String,
    block_number: u64,
) -> anyhow::Result<()> {
    let block_validation = |remote_header: &BlockHeader, local_header: &PartialHeader| {
        assert_eq!(remote_header.extra_data, local_header.extra_data);
        assert_eq!(remote_header.base_fee_per_gas, local_header.base_fee);
        Ok(())
    };
    // Two blocks before the activation point shouldn't see any modification
    assert_replay_header::<OpChainSpec>(
        runtime.clone(),
        url.clone(),
        block_number - 2,
        header_overrides,
        block_validation,
    )
    .await?;

    // One block before the activation point should have a different `extra_data`
    // field than its parent
    assert_replay_header::<OpChainSpec>(
        runtime.clone(),
        url.clone(),
        block_number - 1,
        header_overrides,
        block_validation,
    )
    .await?;

    // The activation point block should use the new values for calculating the base
    // fee
    assert_replay_header::<OpChainSpec>(
        runtime,
        url,
        block_number,
        header_overrides,
        block_validation,
    )
    .await
}

impl_test_dynamic_base_fee_params! {
    op_mainnet: json_rpc_url_provider::op_mainnet() => [
        135_513_416,
        136_165_876,
        144_546_703, // jovian activated block
    ],
    base_mainnet: json_rpc_url_provider::base_mainnet() => [
        25_955_889,
        30_795_009,
        31_747_084,
        37_483_302,
        38_088_319,
        38_951_425, // jovian activated block
        39_647_879, // SystemConfig EIP-1559 update 2025-12-18
        41_711_238, // SystemConfig EIP-1559 update 2026-02-04
    ],
    op_sepolia: json_rpc_url_provider::op_sepolia() => [
        26_806_602,
    ],
    base_sepolia: json_rpc_url_provider::base_sepolia() => [
        21_256_270,
        26_299_084,
    ],
}
