#![cfg(feature = "test-remote")]

use edr_eth::block::{Header, PartialHeader};
use edr_evm::test_utils::assert_replay_header;
use edr_op::OpChainSpec;
use edr_provider::test_utils::header_overrides;

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
                        let url = $url;
                        assert_base_fee_activation(
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

async fn assert_base_fee_activation(url: String, block_number: u64) -> anyhow::Result<()> {
    let block_validation = |remote_header: &Header, local_header: &PartialHeader| {
        assert_eq!(remote_header.extra_data, local_header.extra_data);
        assert_eq!(remote_header.base_fee_per_gas, local_header.base_fee);
        Ok(())
    };
    assert_replay_header::<OpChainSpec>(
        url.clone(),
        block_number - 1,
        header_overrides,
        block_validation,
    )
    .await?;
    assert_replay_header::<OpChainSpec>(url, block_number, header_overrides, block_validation).await
}

impl_test_dynamic_base_fee_params! {
    op_mainnet: op::mainnet_url() => [
        135_513_416,
        136_165_876,
    ],
    base_mainnet: base::mainnet_url() => [
        25_955_889,
        30_795_009,
        31_747_084,
    ],
    op_sepolia: op::sepolia_url() => [
        26_806_602,
    ],
    base_sepolia: base::sepolia_url() => [
        21_256_270,
        26_299_084,
    ],
}
