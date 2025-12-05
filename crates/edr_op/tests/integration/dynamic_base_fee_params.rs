#![cfg(feature = "test-remote")]

use edr_block_header::{BlockHeader, PartialHeader};
use edr_op::OpChainSpec;
use edr_provider::test_utils::header_overrides;
use edr_test_block_replay::assert_replay_header;

use crate::integration::{base, op};

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
    assert_replay_header::<OpChainSpec>(
        runtime.clone(),
        url.clone(),
        block_number - 1,
        header_overrides,
        block_validation,
    )
    .await?;

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
    op_mainnet: op::mainnet_url() => [
        135_513_416,
        136_165_876,
        144_546_703, // jovian activated block
    ],
    base_mainnet: base::mainnet_url() => [
        25_955_889,
        30_795_009,
        31_747_084,
        37_483_302,
        38_088_319,
        38_951_425, // jovian activated block
    ],
    op_sepolia: op::sepolia_url() => [
        26_806_602,
    ],
    base_sepolia: base::sepolia_url() => [
        21_256_270,
        26_299_084,
    ],
}
