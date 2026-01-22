//! tests for `eth_getProof`

use std::{collections::BTreeMap, str::FromStr, sync::Arc};

use alloy_rpc_types::EIP1186AccountProofResponse;
use anyhow::anyhow;
use edr_chain_l1::{Hardfork, L1ChainSpec};
use edr_eth::BlockSpec;
use edr_primitives::{address, Address, Bytes, StorageKey, U256};
use edr_provider::{
    test_utils::{create_test_config_with, MinimalProviderConfig},
    time::CurrentTime,
    MethodInvocation, NoopLogger, Provider, ProviderRequest,
};
use edr_solidity::contract_decoder::ContractDecoder;
use tokio::runtime;

fn setup_provider(
    provider_config: MinimalProviderConfig<Hardfork>,
) -> anyhow::Result<Provider<L1ChainSpec, CurrentTime>> {
    let config = create_test_config_with(provider_config);
    let logger = Box::new(NoopLogger::<L1ChainSpec>::default());
    let subscriber = Box::new(|_event| {});
    Provider::new(
        runtime::Handle::current(),
        logger,
        subscriber,
        config,
        Arc::<ContractDecoder>::default(),
        CurrentTime,
    )
    .map_err(|error| anyhow!(error))
}

fn verify_account_proof<'proof>(
    provider: &Provider<L1ChainSpec, CurrentTime>,
    address: Address,
    block_spec: BlockSpec,
    proof: impl IntoIterator<Item = &'proof str>,
) {
    let expected_proof = proof
        .into_iter()
        .map(Bytes::from_str)
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    let proof_response = provider
        .handle_request(ProviderRequest::with_single(MethodInvocation::GetProof(
            address,
            Vec::new(),
            block_spec,
        )))
        .unwrap();

    let account_proof_response: EIP1186AccountProofResponse =
        serde_json::from_value(proof_response.result)
            .expect("Failed to deserialize account proof response");

    assert_eq!(account_proof_response.account_proof, expected_proof);
}

fn verify_storage_proof<'proof>(
    provider: &Provider<L1ChainSpec, CurrentTime>,
    address: Address,
    slot: StorageKey,
    proof: impl IntoIterator<Item = &'proof str>,
) {
    let expected_proof = proof
        .into_iter()
        .map(Bytes::from_str)
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    let proof_response = provider
        .handle_request(ProviderRequest::with_single(MethodInvocation::GetProof(
            address,
            vec![slot],
            edr_eth::BlockSpec::Tag(edr_eth::BlockTag::Latest),
        )))
        .unwrap();

    let account_proof_response: EIP1186AccountProofResponse =
        serde_json::from_value(proof_response.result)
            .expect("Failed to deserialize account proof response");

    assert_eq!(
        account_proof_response
            .storage_proof
            .first()
            .expect("Storage proof should exist")
            .proof,
        expected_proof
    );
}

mod local {
    use super::*;

    /// Ported from foundry `https://github.com/foundry-rs/foundry/blob/f8904bd/crates/anvil/tests/it/proof.rs`
    /// under the MIT and Apache Licenses
    /// - `https://github.com/foundry-rs/foundry/blob/f8904bd/LICENSE-APACHE`
    /// - `https://github.com/foundry-rs/foundry/blob/f8904bd/LICENSE-MIT`
    #[tokio::test(flavor = "multi_thread")]
    async fn test_account_proof() -> anyhow::Result<()> {
        let provider = setup_provider(MinimalProviderConfig::local_empty())?;

        provider.handle_request(ProviderRequest::with_single(MethodInvocation::SetBalance(
            address!("0x2031f89b3ea8014eb51a78c316e42af3e0d7695f"),
            U256::from(45000000000000000000_u128),
        )))?;
        provider.handle_request(ProviderRequest::with_single(MethodInvocation::SetBalance(
            address!("0x33f0fc440b8477fcfbe9d0bf8649e7dea9baedb2"),
            U256::from(1),
        )))?;
        provider.handle_request(ProviderRequest::with_single(MethodInvocation::SetBalance(
            address!("0x62b0dd4aab2b1a0a04e279e2b828791a10755528"),
            U256::from(1100000000000000000_u128),
        )))?;
        provider.handle_request(ProviderRequest::with_single(MethodInvocation::SetBalance(
            address!("0x1ed9b1dd266b607ee278726d324b855a093394a6"),
            U256::from(120000000000000000_u128),
        )))?;

        verify_account_proof(&provider, address!("0x2031f89b3ea8014eb51a78c316e42af3e0d7695f"), edr_eth::BlockSpec::Tag(edr_eth::BlockTag::Latest), [
            "0xe48200a7a040f916999be583c572cc4dd369ec53b0a99f7de95f13880cf203d98f935ed1b3",
            "0xf87180a04fb9bab4bb88c062f32452b7c94c8f64d07b5851d44a39f1e32ba4b1829fdbfb8080808080a0b61eeb2eb82808b73c4ad14140a2836689f4ab8445d69dd40554eaf1fce34bc080808080808080a0dea230ff2026e65de419288183a340125b04b8405cc61627b3b4137e2260a1e880",
            "0xf8719f31355ec1c8f7e26bb3ccbcb0b75d870d15846c0b98e5cc452db46c37faea40b84ff84d80890270801d946c940000a056e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421a0c5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470"
        ]);

        verify_account_proof(&provider, address!("0x33f0fc440b8477fcfbe9d0bf8649e7dea9baedb2"), edr_eth::BlockSpec::Tag(edr_eth::BlockTag::Latest), [
            "0xe48200a7a040f916999be583c572cc4dd369ec53b0a99f7de95f13880cf203d98f935ed1b3",
            "0xf87180a04fb9bab4bb88c062f32452b7c94c8f64d07b5851d44a39f1e32ba4b1829fdbfb8080808080a0b61eeb2eb82808b73c4ad14140a2836689f4ab8445d69dd40554eaf1fce34bc080808080808080a0dea230ff2026e65de419288183a340125b04b8405cc61627b3b4137e2260a1e880",
            "0xe48200d3a0ef957210bca5b9b402d614eb8408c88cfbf4913eb6ab83ca233c8b8f0e626b54",
            "0xf851808080a02743a5addaf4cf9b8c0c073e1eaa555deaaf8c41cb2b41958e88624fa45c2d908080808080a0bfbf6937911dfb88113fecdaa6bde822e4e99dae62489fcf61a91cb2f36793d680808080808080",
            "0xf8679e207781e762f3577784bab7491fcc43e291ce5a356b9bc517ac52eed3a37ab846f8448001a056e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421a0c5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470",
        ]);

        verify_account_proof(&provider, address!("0x62b0dd4aab2b1a0a04e279e2b828791a10755528"), edr_eth::BlockSpec::Tag(edr_eth::BlockTag::Latest), [
            "0xe48200a7a040f916999be583c572cc4dd369ec53b0a99f7de95f13880cf203d98f935ed1b3",
            "0xf87180a04fb9bab4bb88c062f32452b7c94c8f64d07b5851d44a39f1e32ba4b1829fdbfb8080808080a0b61eeb2eb82808b73c4ad14140a2836689f4ab8445d69dd40554eaf1fce34bc080808080808080a0dea230ff2026e65de419288183a340125b04b8405cc61627b3b4137e2260a1e880",
            "0xf8709f3936599f93b769acf90c7178fd2ddcac1b5b4bc9949ee5a04b7e0823c2446eb84ef84c80880f43fc2c04ee0000a056e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421a0c5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470",
        ]);

        verify_account_proof(&provider, address!("0x1ed9b1dd266b607ee278726d324b855a093394a6"), edr_eth::BlockSpec::Tag(edr_eth::BlockTag::Latest), [
            "0xe48200a7a040f916999be583c572cc4dd369ec53b0a99f7de95f13880cf203d98f935ed1b3",
            "0xf87180a04fb9bab4bb88c062f32452b7c94c8f64d07b5851d44a39f1e32ba4b1829fdbfb8080808080a0b61eeb2eb82808b73c4ad14140a2836689f4ab8445d69dd40554eaf1fce34bc080808080808080a0dea230ff2026e65de419288183a340125b04b8405cc61627b3b4137e2260a1e880",
            "0xe48200d3a0ef957210bca5b9b402d614eb8408c88cfbf4913eb6ab83ca233c8b8f0e626b54",
            "0xf851808080a02743a5addaf4cf9b8c0c073e1eaa555deaaf8c41cb2b41958e88624fa45c2d908080808080a0bfbf6937911dfb88113fecdaa6bde822e4e99dae62489fcf61a91cb2f36793d680808080808080",
            "0xf86f9e207a32b8ab5eb4b043c65b1f00c93f517bc8883c5cd31baf8e8a279475e3b84ef84c808801aa535d3d0c0000a056e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421a0c5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470"
        ]);

        Ok(())
    }

    /// Ported from foundry `https://github.com/foundry-rs/foundry/blob/f8904bd/crates/anvil/tests/it/proof.rs`
    /// under the MIT and Apache Licenses
    /// - `https://github.com/foundry-rs/foundry/blob/f8904bd/LICENSE-APACHE`
    /// - `https://github.com/foundry-rs/foundry/blob/f8904bd/LICENSE-MIT`
    #[tokio::test(flavor = "multi_thread")]
    async fn test_storage_proof() -> anyhow::Result<()> {
        let target = address!("0x1ed9b1dd266b607ee278726d324b855a093394a6");

        let provider = setup_provider(MinimalProviderConfig::local_empty())?;

        let storage: BTreeMap<U256, U256> =
            serde_json::from_str(include_str!("../fixtures/storage_sample.json")).unwrap();

        for (key, value) in storage {
            provider.handle_request(ProviderRequest::with_single(
                MethodInvocation::SetStorageAt(target, key, value),
            ))?;
        }

        verify_storage_proof(&provider, target, StorageKey::from_str("0000000000000000000000000000000000000000000000000000000000000022")?, [
        "0xf9019180a0aafd5b14a6edacd149e110ba6776a654f2dbffca340902be933d011113f2750380a0a502c93b1918c4c6534d4593ae03a5a23fa10ebc30ffb7080b297bff2446e42da02eb2bf45fd443bd1df8b6f9c09726a4c6252a0f7896a131a081e39a7f644b38980a0a9cf7f673a0bce76fd40332afe8601542910b48dea44e93933a3e5e930da5d19a0ddf79db0a36d0c8134ba143bcb541cd4795a9a2bae8aca0ba24b8d8963c2a77da0b973ec0f48f710bf79f63688485755cbe87f9d4c68326bb83c26af620802a80ea0f0855349af6bf84afc8bca2eda31c8ef8c5139be1929eeb3da4ba6b68a818cb0a0c271e189aeeb1db5d59d7fe87d7d6327bbe7cfa389619016459196497de3ccdea0e7503ba5799e77aa31bbe1310c312ca17b2c5bcc8fa38f266675e8f154c2516ba09278b846696d37213ab9d20a5eb42b03db3173ce490a2ef3b2f3b3600579fc63a0e9041059114f9c910adeca12dbba1fef79b2e2c8899f2d7213cd22dfe4310561a047c59da56bb2bf348c9dd2a2e8f5538a92b904b661cfe54a4298b85868bbe4858080",
        "0xf85180a0776aa456ba9c5008e03b82b841a9cf2fc1e8578cfacd5c9015804eae315f17fb80808080808080808080808080a072e3e284d47badbb0a5ca1421e1179d3ea90cc10785b26b74fb8a81f0f9e841880",
        "0xf843a020035b26e3e9eee00e0d72fd1ee8ddca6894550dca6916ea2ac6baa90d11e510a1a0f5a5fd42d16a20302798ef6ed309979b43003d2320d9f0e8ea9831a92759fb4b"
    ]);

        verify_storage_proof(&provider, target, StorageKey::from_str("0000000000000000000000000000000000000000000000000000000000000023")?, [
        "0xf9019180a0aafd5b14a6edacd149e110ba6776a654f2dbffca340902be933d011113f2750380a0a502c93b1918c4c6534d4593ae03a5a23fa10ebc30ffb7080b297bff2446e42da02eb2bf45fd443bd1df8b6f9c09726a4c6252a0f7896a131a081e39a7f644b38980a0a9cf7f673a0bce76fd40332afe8601542910b48dea44e93933a3e5e930da5d19a0ddf79db0a36d0c8134ba143bcb541cd4795a9a2bae8aca0ba24b8d8963c2a77da0b973ec0f48f710bf79f63688485755cbe87f9d4c68326bb83c26af620802a80ea0f0855349af6bf84afc8bca2eda31c8ef8c5139be1929eeb3da4ba6b68a818cb0a0c271e189aeeb1db5d59d7fe87d7d6327bbe7cfa389619016459196497de3ccdea0e7503ba5799e77aa31bbe1310c312ca17b2c5bcc8fa38f266675e8f154c2516ba09278b846696d37213ab9d20a5eb42b03db3173ce490a2ef3b2f3b3600579fc63a0e9041059114f9c910adeca12dbba1fef79b2e2c8899f2d7213cd22dfe4310561a047c59da56bb2bf348c9dd2a2e8f5538a92b904b661cfe54a4298b85868bbe4858080",
        "0xf8518080808080a0d546c4ca227a267d29796643032422374624ed109b3d94848c5dc06baceaee76808080808080a027c48e210ccc6e01686be2d4a199d35f0e1e8df624a8d3a17c163be8861acd6680808080",
        "0xf843a0207b2b5166478fd4318d2acc6cc2c704584312bdd8781b32d5d06abda57f4230a1a0db56114e00fdd4c1f85c892bf35ac9a89289aaecb1ebd0a96cde606a748b5d71"
    ]);

        verify_storage_proof(&provider, target, StorageKey::from_str("0000000000000000000000000000000000000000000000000000000000000024")?, [
        "0xf9019180a0aafd5b14a6edacd149e110ba6776a654f2dbffca340902be933d011113f2750380a0a502c93b1918c4c6534d4593ae03a5a23fa10ebc30ffb7080b297bff2446e42da02eb2bf45fd443bd1df8b6f9c09726a4c6252a0f7896a131a081e39a7f644b38980a0a9cf7f673a0bce76fd40332afe8601542910b48dea44e93933a3e5e930da5d19a0ddf79db0a36d0c8134ba143bcb541cd4795a9a2bae8aca0ba24b8d8963c2a77da0b973ec0f48f710bf79f63688485755cbe87f9d4c68326bb83c26af620802a80ea0f0855349af6bf84afc8bca2eda31c8ef8c5139be1929eeb3da4ba6b68a818cb0a0c271e189aeeb1db5d59d7fe87d7d6327bbe7cfa389619016459196497de3ccdea0e7503ba5799e77aa31bbe1310c312ca17b2c5bcc8fa38f266675e8f154c2516ba09278b846696d37213ab9d20a5eb42b03db3173ce490a2ef3b2f3b3600579fc63a0e9041059114f9c910adeca12dbba1fef79b2e2c8899f2d7213cd22dfe4310561a047c59da56bb2bf348c9dd2a2e8f5538a92b904b661cfe54a4298b85868bbe4858080",
        "0xf85180808080a030263404acfee103d0b1019053ff3240fce433c69b709831673285fa5887ce4c80808080808080a0f8f1fbb1f7b482d9860480feebb83ff54a8b6ec1ead61cc7d2f25d7c01659f9c80808080",
        "0xf843a020d332d19b93bcabe3cce7ca0c18a052f57e5fd03b4758a09f30f5ddc4b22ec4a1a0c78009fdf07fc56a11f122370658a353aaa542ed63e44c4bc15ff4cd105ab33c",
    ]);

        verify_storage_proof(&provider, target, StorageKey::from_str("0000000000000000000000000000000000000000000000000000000000000100")?, [
        "0xf9019180a0aafd5b14a6edacd149e110ba6776a654f2dbffca340902be933d011113f2750380a0a502c93b1918c4c6534d4593ae03a5a23fa10ebc30ffb7080b297bff2446e42da02eb2bf45fd443bd1df8b6f9c09726a4c6252a0f7896a131a081e39a7f644b38980a0a9cf7f673a0bce76fd40332afe8601542910b48dea44e93933a3e5e930da5d19a0ddf79db0a36d0c8134ba143bcb541cd4795a9a2bae8aca0ba24b8d8963c2a77da0b973ec0f48f710bf79f63688485755cbe87f9d4c68326bb83c26af620802a80ea0f0855349af6bf84afc8bca2eda31c8ef8c5139be1929eeb3da4ba6b68a818cb0a0c271e189aeeb1db5d59d7fe87d7d6327bbe7cfa389619016459196497de3ccdea0e7503ba5799e77aa31bbe1310c312ca17b2c5bcc8fa38f266675e8f154c2516ba09278b846696d37213ab9d20a5eb42b03db3173ce490a2ef3b2f3b3600579fc63a0e9041059114f9c910adeca12dbba1fef79b2e2c8899f2d7213cd22dfe4310561a047c59da56bb2bf348c9dd2a2e8f5538a92b904b661cfe54a4298b85868bbe4858080",
        "0xf891a090bacef44b189ddffdc5f22edc70fe298c58e5e523e6e1dfdf7dbc6d657f7d1b80a026eed68746028bc369eb456b7d3ee475aa16f34e5eaa0c98fdedb9c59ebc53b0808080a09ce86197173e14e0633db84ce8eea32c5454eebe954779255644b45b717e8841808080a0328c7afb2c58ef3f8c4117a8ebd336f1a61d24591067ed9c5aae94796cac987d808080808080",
    ]);

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_can_get_random_account_proofs() -> anyhow::Result<()> {
        let provider = setup_provider(MinimalProviderConfig::local_empty())?;

        let address = Address::random();
        provider
            .handle_request(ProviderRequest::with_single(MethodInvocation::GetProof(
                address,
                Vec::new(),
                edr_eth::BlockSpec::Tag(edr_eth::BlockTag::Latest),
            )))
            .unwrap_or_else(|_| panic!("Failed to get proof for {address:?}"));

        Ok(())
    }
}
#[cfg(feature = "test-remote")]
mod fork {
    use edr_primitives::HashMap;
    use edr_provider::{ForkConfig, ProviderError};
    use edr_state_api::StateError;
    use edr_test_utils::env::json_rpc_url_provider;

    use super::*;

    const FORK_BLOCK_NUMBER: u64 = 24249944;
    const ADDRESS: Address = address!("0xe44342c0ccb1c53cc85468aa4759cc8956c38933"); // random address hardcoded for caching purposes

    #[tokio::test(flavor = "multi_thread")]
    async fn test_get_proof_for_local_block_fails() -> anyhow::Result<()> {
        let provider = setup_provider(MinimalProviderConfig::fork_empty(ForkConfig {
            block_number: Some(FORK_BLOCK_NUMBER),
            cache_dir: edr_defaults::CACHE_DIR.into(),
            chain_overrides: HashMap::default(),
            http_headers: None,
            url: json_rpc_url_provider::ethereum_mainnet(),
        }))?;

        provider.handle_request(ProviderRequest::with_single(MethodInvocation::SetBalance(
            ADDRESS,
            U256::from(100_000),
        )))?;

        let proof_result =
            provider.handle_request(ProviderRequest::with_single(MethodInvocation::GetProof(
                ADDRESS,
                Vec::new(),
                edr_eth::BlockSpec::Tag(edr_eth::BlockTag::Latest),
            )));

        assert!(matches!(
            proof_result.unwrap_err(),
            ProviderError::State(StateError::Unsupported { .. })
        ));

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_get_proof_for_fork_block_works() -> anyhow::Result<()> {
        let provider = setup_provider(MinimalProviderConfig::fork_empty(ForkConfig {
            block_number: Some(FORK_BLOCK_NUMBER),
            cache_dir: edr_defaults::CACHE_DIR.into(),
            chain_overrides: HashMap::default(),
            http_headers: None,
            url: json_rpc_url_provider::ethereum_mainnet(),
        }))?;

        verify_account_proof(&provider, ADDRESS, BlockSpec::Number(FORK_BLOCK_NUMBER),
            // proof obtained from Alchemy eth_getProof call
            [
                "0xf90211a04578c172f16e4fa4456d7bd41cc26acbdb583fd5d4f65878d9b4eb1515e8101ca0251f5be07963da539f46d774dce6fb85ce3b5c750e7861bfd7025b105ad0b85fa0518a6d763d459519e8d6b355c95cc9fed7fcba0ddb9ed5db0f52b4aefc37fd0ca09f5d9df287c192b924e82bbca03f7592dca8201adfc59a9a487f4184dca4a467a0f34e42b293583c2a2fe4e75c6cfde479af85c7bbfaad7b3e3c8d27bb42f55551a0277c036db994dab3bbd844191cdd8ca80316d44ac92414f43ff6c4d4bf49887ba0fc56ac0e4236e9dc0af0b23ee962b58a1600a44303ec7691c732026dffb8033ba07a3e54b3f2338bf2612cf7a96afdfaf97d80fecb4083f48a6c179e314cad94a9a07038a84c2c67af8384db50774de73f74820ecd3118b503efd22a1847791d91f4a07bcb37c53ee51facb203b0b984e06ae6f301a4e4859d396700bbde0b56b5f654a06e8b0e162089e403507dc4dc2251e9a952cff9edc74f58ef485a0a1887183525a047b240528c2fdc2dbaea65f37f9b2c295303fb1aca7857378897f91118869f06a03b28651d90db3809f7242f002b278336b093fad3701bad61fc76b7338307066fa00b91fcbe9be542e6e958d088a535a88584ccf14081cb78e335179fa1b326d4daa06747a1be1788705b64769fb866bac4b6d852a3802a76b67f3752c96d592fb983a0c227d4647759f81ddd5e3863c99a246dfc1a0ff0a7696cba0d57b86e6a4c5b2e80",
                "0xf90211a0157063dbe845ed854684994b482355e0e947958701504e4773dda63052487842a0f07dd9290baa321dd626d38b14311efb17aee86d926e13670f9e3dd6b0a0e66da039cc3cf78bc4e3b4b6e93b0044510cb51793284e5091f721e53f4f300438f052a0396a0c9b7ba28052336340eaed5cb127e099f2019b81ab5b11eb8ae610a9e415a0bc3f2bb42c7fad61f9eef58cd3924ada64ebdc231fb62e65c98576b2f6272d83a08c32601e36761a8117bcf9e2ed47c8b21be09b9f5e0d91b2c7689f3ffa78ef3aa02c81a56a7b5a4ed1949b9b01b3faa2054164fd5c90edad90aba9267f8a79f656a0dfad6815c4fecb86a72ee74df745999fa9024a33fa3f73447214af1e066ffb39a06df2bdcbfebcc2812165fab168f00cca054533791e1a4f2ff21217112bfc6ab6a07e73d022516ed6505bcc7e2f52ed1daced4ae7bec920b5c5fd158416351459b2a0e0e9fa4b54cf6c310367f1829d32e1488d16420c281a86c72b893e7f5bf5bfa8a029d6fad677098d3ebee6e24cb4d6978e2de095c3367db5efdcab7557127e8dd8a0ab6daf9d7f15819e788994f04a673e36ba0b35bde8da9d6f716aa3b303292022a0308692c6546e4ffa1de4ba4f065739093412eced2a1ab77a11ec4b4df23bb5cfa06cc8be5077590459c56e4203c7c575e0c95b808ddd4c24a903c4a672b5a0e954a0d6755942259d9f6cc9a135d441041c9be29d986477f6f07d0a427644f0cb39e680",
                "0xf90211a099c5e88990c83fe16b39e0f74734a5e13b26c76fc636b0b4a4d41ff550489108a066d1d7156f1b5117030df1ca31d4eac6b79f39739368eebe742f9b1211ec3bc4a0a8a5f02809c5860ee9bf9a3145cd5e129052a8f20867ab6ba5c1e59240e858e4a06e59621254f1c11ce5f984d6ec3442dbf8834f710044a3049d3baa1d32df59e3a019aedc36559df6d4d7dde824af879d596fb08cf794131f17191773eb2a7a0516a052ab65a6ca4db9fbcb4f1d45f001d6e0bcceb3d892a4e86b3e05768e6470dabca0c4d3f88ca507a5fd5d16fa96c2ecde97f77ebbb8d59171ed4a4827a8a73ab3c4a05e3ddd097ecd2a53d92e29d1c969a6dd0f688807b0879bb8d56121a370d2aef5a0d7b2e0ee1048587843aeb3ee125a2d2d5ae4133c5d55c51dc8cb9a99a7e2df8fa0594786a979024f0aed519be02fb47ab5ac2c890cb48fc8ea83fb42a29165ee53a068224651925bc9976af03a34a4a575ea96867cc955189257bc291d3f78a5cea4a08ce4659712ec0162e9c118dd88e465c5b41998321c38e550e22671a6af05a56ba0bacd5d07cb0058ba0c2c7dd828d6d142b6e81bc9ab0edad014cfcb4fd74f59e7a054a562fc1d29ee5a0ce5dc049d49dd941c719a5c19cd78630e43c53ba420cbd1a007d60c6f82eccba737a562ebc976463646ca76e32407174839c428241b67ddc1a0c81fc28b628ac12f4d16db7ea97290776eccd9de66ce4c4a06bd9c355df5c89380",
                "0xf90211a06c7ed6fae37b3fa435dbc6e9d967f0794c7b2216e4856ae22b51aab41b0463bda02d30c29571aaedafb510944553b3c7ca5044ee656217bfb677271c2e88d93950a0e8bce5430da5365d9dca1ffaca3013162f1aa1a7274e389f224cdeb3d96a51b5a0debb16611dda2f29a482e24a1da0ff9cfedde99f0c10a271077789ff567faf80a0a6be18b3bab1e7ad7d68c61a49f48f8355bbd29fd92c000ecc24d54f509ed45aa02894f11168e37ccf25d8b2f52b6623eb4071427498c9b3954688afeba3af70c8a0aba0881c72a5eb7508d82368d1297889056075d2b591d6b2b75c2934b44f7658a01570b7f43da943dc5f2415651c46cf5e1c2744839568d4ddde8e47dd546f2a7fa07a04e4b021c1e60fab0ae99393821844151786ead13019a7b8d6cdd1ebc04347a0efe5b197a93961b40c00001f3346e1db4bfb4b02aa1d0d2ed963c1843634997aa0d1716c20dee8d9e69b8a1dae0503854d4c2b714ee00e4dddc7a587159fcbec2ca04f66e806c16a6dc80a4cf50fa767a0a48f97d3068e9289da9706299fc006d3b1a00da999ef45b3ff25b58b6651a5de30d11df8123fe5ad0dad40a101bd56235281a0f189f6119bd2b1eff1f5da17938fda38bf8a512a4bededf41b014a83c79bfc79a09e69e819fdace598a77a15432f73145b5c5063a03bb528459095f049902e99b0a03acdc54794cfef6c48a25611623fcc404fbdffec246ba83f312907caf825933780",
                "0xf90211a0d3a4695731a5270ab112a9218056825cff4872355844048535d1eb73176b93c6a00b0a1c375fb506442349ce25db5d65057f3162ff22a873a6aebf4e479bf1e18aa0afb11a37d5deb61e339f3fae41c4cee26f9e211bbed046238772169100c441e0a0c28d4037ff24332a1a05f84d6a74d1521677b60dbb38a081f7e1af273ceee83ea0cb943cf12ae1959e93543f264481b22db2cdcfc25d355fcce5bdef6bb791fbc7a0ec69e0f4f941d10f3001bfacc4cc56e589bfcc9e05d8692c9a4185c2415ef26ca04b74296be3f998cdf881f797254e3d61f3f15f87b330c2e627d5ca96af8e482ea013bfbbc59b0fe5e77dfcdd3c79b519d7d2cea24daa486dbedba2cd6ef5c1d390a0dcc148f10d697cd4d26dca579232580f21053473da940c5871ea4d4d71c322cda03f0fd9d194b5b7413db6ed6d567bde73b1daa4abbd82277ab751c6b3f1343f95a0f2d2ae6c43839dcbf3b81b92549fbfec15306e39090e5b17f71ea2cb357106c6a0cea753ad7124f8b208bf4804a6ea9d417d4ecd91e91d202dea824fda2edc6569a0d8004eae1cf19bf2d5b218a769ed4fcb5b8207169b1d7374c0ff3c1ff6248724a0ff3895d286c7f3e4ec6a1303e44e82e9f288aad423c8c6ffcaf2f1f582e9e357a06d3f3b31e065c633ddddb4031dac5a74a8fd8f1493df7e065d72e67d0a1ba65fa0e11cdccc61274056c39e7ac188bbc6c484a34fd8a75fc69330b2831038aea55a80",
                "0xf90211a06da132f7e64a41a50a276e017068bb30be53e17fa77d5ebbb721e7f0c37681a4a0a92d4ee08606231968b3202daab8bc7179ad5c2ad4f4f0e7c57d4cf8e8d1b87ea0d895e96c0d389ec4997980e7dfc0b2d59427dd3af15c31f25bad841d41487847a06c8cee1c2dd39829b6a97e29d5415a71d94dbb63d80095f3c7b2defd61abdcc5a0c4c1bf755cedee90d63adf3cd4fd9492931a02e887d44103f163e1aa58a9cc36a0a293402820a12198ae3dd3b9a71d02c43cc39a59089dba51ffffebe36a909431a0fc44db787f4078d72e7f5de5f3100993fd869f9037415cb37ed13e60f25ad890a0c1cdbdaa6a000f632e960a394ed96ad62cb8a27f4bb62ae21cc782c0af12737aa0b8497ed6b3507da6a4292e6201d74ad290319589001d4418b2055271a9a6e92fa0095aad802ba56641f6db8729d18aed9e8baacb39cbb82d50661cf05a76b80470a07bf8deee4fd5d2fb2ed2ad6a99b0498edc3813fd740c49329f7056698814935da0e86604d6f5d372a69d71508028162cbac71a5693b9791a0394a7e1ba43a6ee28a04ce5cc541e6e201edf7825ac3ec09b270e66d0d855c9b387bea03a1996a4b32aa0d6581d89bcdedcc19cca320dca0e3395640a6f24f9ce262de1b8671a7fb43842a00012151af46985a9e583e9553f8f49bc24bd33a1df6a0db5edf6118e158fd73aa0bbc79c4b5d93439c3c61ffd0b1a703d9eb7dfa552f4bf2ae14c048bdc750ba9880",
                "0xf901d1a095506fef4e979268f00ea8dbb5b4617c0157ae4de2efcbf15469be5cf4597846a08eddf70d4dc30906b3da63ca9c0866f9118250eef5a7f9b2ef6e8345d8748ea6a04fec185b10c60d84d79f9e17c7e334dfa3c82432a4c85622f2f5b955bb665b91a04f5a306c13214c119f44fc3b45c1db1177cd11418a0bfc454342ddb6b30d70f4a009ae290d6c16fdff4ca4fbfbc4e6c7e61f53f450ad131e2b9b93acb79e87f195a0a62bc61a2f079ec4143d423feecd80eb5e297556d6dc016091da262686f03553a07952347da7107711abc18be43ef96aaa07a1ebaaae88fa375652272437615ee1a0773aa3fb602c21b5825a66b069a252c36ed86056fa841249234ef1869cc8c271a07e58d5e199df1e237781f7b14fa5bd5a31ec4f52b62e7bdbea3211ecf3ee2637a05dc2e5292b687c0d1f41f73d040079203c956af923ec4881a237aa016ae254a3a0a0b56fffc45d9a261bd1f1dd631d74a3cf1dc5ad05aac40df703aba30cdb716ea0abb4068f04affd23a397c999c56fc1f9117a9422f162552e93a7d26abc3bd345a094ef7f00e9f21b4b691be4f1795becb54049b37828e100e9c67b9e38e6ae09e08080a0df80f9bde4c8c04f7c6757438f1acffa0affb75d48b5e85049c25dfac10dc4ff80",
                "0xf86c9d333b108031527c38b3550879600b92d4b747494ed4af1b3af6ffdbcf78b84cf84a018633d758c09000a056e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421a0c5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470"
            ]);
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_get_proof_for_fork_block_with_genesis_state_fails() -> anyhow::Result<()> {
        let provider = setup_provider(MinimalProviderConfig::fork_with_accounts(ForkConfig {
            block_number: Some(FORK_BLOCK_NUMBER),
            cache_dir: edr_defaults::CACHE_DIR.into(),
            chain_overrides: HashMap::default(),
            http_headers: None,
            url: json_rpc_url_provider::ethereum_mainnet(),
        }))?;

        let proof_result =
            provider.handle_request(ProviderRequest::with_single(MethodInvocation::GetProof(
                ADDRESS,
                Vec::new(),
                edr_eth::BlockSpec::Tag(edr_eth::BlockTag::Latest),
            )));

        assert!(matches!(
            proof_result.unwrap_err(),
            ProviderError::State(StateError::Unsupported { .. })
        ));
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_get_proof_for_previous_fork_block_works() -> anyhow::Result<()> {
        let provider = setup_provider(MinimalProviderConfig::fork_empty(ForkConfig {
            block_number: Some(FORK_BLOCK_NUMBER),
            cache_dir: edr_defaults::CACHE_DIR.into(),
            chain_overrides: HashMap::default(),
            http_headers: None,
            url: json_rpc_url_provider::ethereum_mainnet(),
        }))?;
        verify_account_proof(&provider, ADDRESS, BlockSpec::Number(FORK_BLOCK_NUMBER - 1),
            // proof obtained from Alchemy eth_getProof call
            [
                "0xf90211a0d1350b292ec4abf1f85464a3fb229d26b44a34a587339071d031fafc0eeca724a036157ecb79e7748cdfbd1d41f753fb902e902c80ad32bf81bd1ed8ab0ac949b2a00e86be3f1c4125b4831bb791c05c17d863eeb8edf4272e6c68e62557d12f750ba01687e6f3faa6151b4e25740896c83cbe440db3070c320eeae609a08f5c6e5a2da017579b44dae1b1e41282142ef40bc585b8569cc12ca914b4e7f43bee9209be7da0854a9016096ce65023e31bd918dc7ff64a65725091201b41c8f4ae0ddd7c045aa061d0f17b9e30f6bb21be98175247068e3f992609cf8571d1c5293931550c925ea00e26ced4deb44a472ae23127d62e026b147e2cacc0965d63af2b165ac2b28588a0b35c6e8f963e3a095710f546b8a5b710a7a5e85fea733b0c0ba91a18e8dacb7da07e3ce64169e7606b6251ec977a3d6d787c4add21c00fa707f7621958ea6093b8a018ee6c3b58972e8433e4da7906ecf44f158c21889a9002f640f9a96f3c1ae6dda070db91499e4cd691ad323f9d1401669161ff173365e194b7aac8fbb9de26e659a000e541e369c482a96fddafbea9b4672fca04ee04ca6d1b705662e601afcaae0aa0a5d1b411474b6f9788a0e4292d8ea50567658997defe238dd56196863283409ea039d25295afb3836b5e184f99d161340b2c350abca16e1e0c17f8d2106c776b77a0cd9e4de5234e4d7140515627f7d14078b01d6471e2fbc43abd478fb4a75fde6180",
                "0xf90211a099f176f4800bf10cf34b9078fabd7ee15d3a057804f7c2f32da5db96ed429b63a0eb5395967447203a384e35bcb14cea3f8edb1782a6eb8718c966f4b1931f8b37a09bafe9f7ed754b959b47134f5ea364abf9ec51e7f1d28f569653a2ef6722f5a1a0e8f4d8832a1001913b50c1852c5134881f4c616a9e3ab2c1ca56e01d9d74616ba066c967ef7a962f7c056cdbd3878651975303a3c6a56ef7695f0a90b675c4723ba0d9214cae23b1556e9817bd6588dea8993679b92fdc5fda95f5a35e3e98d30081a0c2503727907c2f2660ec8b3953aeb400823ae292a1fdfeb7ff041c22486a6f4da0a80ba3c9faf0f043f000b8c8e8b3b38bb2f24bfb4183e8dc55df77443e98da81a06df2bdcbfebcc2812165fab168f00cca054533791e1a4f2ff21217112bfc6ab6a089651e6f20d1812c3468673d74c1644775ffd4a59f267abf1de31932fbd935caa0e24a026f9004aab0ee0ff94f56754a44c835c3675b67fb29bf1e56f908ac2be5a0dfbd565bd467df94fe2001f1a2148bdfcee1df078e54170fd5b5d0dbc4a28964a0d8528e522e26ab030a72dd41979ebc5881efb127e3313ffce8d79c4b88a410d7a0f1d2ba035698070610f95a3bb7c7cc8c9bf5002e7ffcae2f8a3f3e7f4dcbdd4ba06dcba44018ea02167efc53731fc6e705a774da0f668f43e2fc2006baf6633a85a001d08ee5260b21559b992a9f63c1786476bf353a73288244b60cd9c5fb4851a680",
                "0xf90211a099c5e88990c83fe16b39e0f74734a5e13b26c76fc636b0b4a4d41ff550489108a066d1d7156f1b5117030df1ca31d4eac6b79f39739368eebe742f9b1211ec3bc4a0a8a5f02809c5860ee9bf9a3145cd5e129052a8f20867ab6ba5c1e59240e858e4a06e59621254f1c11ce5f984d6ec3442dbf8834f710044a3049d3baa1d32df59e3a019aedc36559df6d4d7dde824af879d596fb08cf794131f17191773eb2a7a0516a052ab65a6ca4db9fbcb4f1d45f001d6e0bcceb3d892a4e86b3e05768e6470dabca0ea2c30606aa56f63f4e4effe04e8791a6642f9507257cfa31a3288f17f263511a05e3ddd097ecd2a53d92e29d1c969a6dd0f688807b0879bb8d56121a370d2aef5a0d7b2e0ee1048587843aeb3ee125a2d2d5ae4133c5d55c51dc8cb9a99a7e2df8fa0594786a979024f0aed519be02fb47ab5ac2c890cb48fc8ea83fb42a29165ee53a068224651925bc9976af03a34a4a575ea96867cc955189257bc291d3f78a5cea4a08ce4659712ec0162e9c118dd88e465c5b41998321c38e550e22671a6af05a56ba0c44884774a0b3c8e5fa15388555c50a484a0d24ed32d16f0d5c59a37bd3acf02a054a562fc1d29ee5a0ce5dc049d49dd941c719a5c19cd78630e43c53ba420cbd1a007d60c6f82eccba737a562ebc976463646ca76e32407174839c428241b67ddc1a0c81fc28b628ac12f4d16db7ea97290776eccd9de66ce4c4a06bd9c355df5c89380",
                "0xf90211a06c7ed6fae37b3fa435dbc6e9d967f0794c7b2216e4856ae22b51aab41b0463bda02d30c29571aaedafb510944553b3c7ca5044ee656217bfb677271c2e88d93950a0e8bce5430da5365d9dca1ffaca3013162f1aa1a7274e389f224cdeb3d96a51b5a0debb16611dda2f29a482e24a1da0ff9cfedde99f0c10a271077789ff567faf80a0a6be18b3bab1e7ad7d68c61a49f48f8355bbd29fd92c000ecc24d54f509ed45aa02894f11168e37ccf25d8b2f52b6623eb4071427498c9b3954688afeba3af70c8a0aba0881c72a5eb7508d82368d1297889056075d2b591d6b2b75c2934b44f7658a01570b7f43da943dc5f2415651c46cf5e1c2744839568d4ddde8e47dd546f2a7fa07a04e4b021c1e60fab0ae99393821844151786ead13019a7b8d6cdd1ebc04347a0efe5b197a93961b40c00001f3346e1db4bfb4b02aa1d0d2ed963c1843634997aa0d1716c20dee8d9e69b8a1dae0503854d4c2b714ee00e4dddc7a587159fcbec2ca04f66e806c16a6dc80a4cf50fa767a0a48f97d3068e9289da9706299fc006d3b1a00da999ef45b3ff25b58b6651a5de30d11df8123fe5ad0dad40a101bd56235281a0f189f6119bd2b1eff1f5da17938fda38bf8a512a4bededf41b014a83c79bfc79a09e69e819fdace598a77a15432f73145b5c5063a03bb528459095f049902e99b0a03acdc54794cfef6c48a25611623fcc404fbdffec246ba83f312907caf825933780",
                "0xf90211a0d3a4695731a5270ab112a9218056825cff4872355844048535d1eb73176b93c6a00b0a1c375fb506442349ce25db5d65057f3162ff22a873a6aebf4e479bf1e18aa0afb11a37d5deb61e339f3fae41c4cee26f9e211bbed046238772169100c441e0a0c28d4037ff24332a1a05f84d6a74d1521677b60dbb38a081f7e1af273ceee83ea0cb943cf12ae1959e93543f264481b22db2cdcfc25d355fcce5bdef6bb791fbc7a0ec69e0f4f941d10f3001bfacc4cc56e589bfcc9e05d8692c9a4185c2415ef26ca04b74296be3f998cdf881f797254e3d61f3f15f87b330c2e627d5ca96af8e482ea013bfbbc59b0fe5e77dfcdd3c79b519d7d2cea24daa486dbedba2cd6ef5c1d390a0dcc148f10d697cd4d26dca579232580f21053473da940c5871ea4d4d71c322cda03f0fd9d194b5b7413db6ed6d567bde73b1daa4abbd82277ab751c6b3f1343f95a0f2d2ae6c43839dcbf3b81b92549fbfec15306e39090e5b17f71ea2cb357106c6a0cea753ad7124f8b208bf4804a6ea9d417d4ecd91e91d202dea824fda2edc6569a0d8004eae1cf19bf2d5b218a769ed4fcb5b8207169b1d7374c0ff3c1ff6248724a0ff3895d286c7f3e4ec6a1303e44e82e9f288aad423c8c6ffcaf2f1f582e9e357a06d3f3b31e065c633ddddb4031dac5a74a8fd8f1493df7e065d72e67d0a1ba65fa0e11cdccc61274056c39e7ac188bbc6c484a34fd8a75fc69330b2831038aea55a80",
                "0xf90211a06da132f7e64a41a50a276e017068bb30be53e17fa77d5ebbb721e7f0c37681a4a0a92d4ee08606231968b3202daab8bc7179ad5c2ad4f4f0e7c57d4cf8e8d1b87ea0d895e96c0d389ec4997980e7dfc0b2d59427dd3af15c31f25bad841d41487847a06c8cee1c2dd39829b6a97e29d5415a71d94dbb63d80095f3c7b2defd61abdcc5a0c4c1bf755cedee90d63adf3cd4fd9492931a02e887d44103f163e1aa58a9cc36a0a293402820a12198ae3dd3b9a71d02c43cc39a59089dba51ffffebe36a909431a0fc44db787f4078d72e7f5de5f3100993fd869f9037415cb37ed13e60f25ad890a0c1cdbdaa6a000f632e960a394ed96ad62cb8a27f4bb62ae21cc782c0af12737aa0b8497ed6b3507da6a4292e6201d74ad290319589001d4418b2055271a9a6e92fa0095aad802ba56641f6db8729d18aed9e8baacb39cbb82d50661cf05a76b80470a07bf8deee4fd5d2fb2ed2ad6a99b0498edc3813fd740c49329f7056698814935da0e86604d6f5d372a69d71508028162cbac71a5693b9791a0394a7e1ba43a6ee28a04ce5cc541e6e201edf7825ac3ec09b270e66d0d855c9b387bea03a1996a4b32aa0d6581d89bcdedcc19cca320dca0e3395640a6f24f9ce262de1b8671a7fb43842a00012151af46985a9e583e9553f8f49bc24bd33a1df6a0db5edf6118e158fd73aa0bbc79c4b5d93439c3c61ffd0b1a703d9eb7dfa552f4bf2ae14c048bdc750ba9880",
                "0xf901d1a095506fef4e979268f00ea8dbb5b4617c0157ae4de2efcbf15469be5cf4597846a08eddf70d4dc30906b3da63ca9c0866f9118250eef5a7f9b2ef6e8345d8748ea6a04fec185b10c60d84d79f9e17c7e334dfa3c82432a4c85622f2f5b955bb665b91a04f5a306c13214c119f44fc3b45c1db1177cd11418a0bfc454342ddb6b30d70f4a009ae290d6c16fdff4ca4fbfbc4e6c7e61f53f450ad131e2b9b93acb79e87f195a0a62bc61a2f079ec4143d423feecd80eb5e297556d6dc016091da262686f03553a07952347da7107711abc18be43ef96aaa07a1ebaaae88fa375652272437615ee1a0773aa3fb602c21b5825a66b069a252c36ed86056fa841249234ef1869cc8c271a07e58d5e199df1e237781f7b14fa5bd5a31ec4f52b62e7bdbea3211ecf3ee2637a05dc2e5292b687c0d1f41f73d040079203c956af923ec4881a237aa016ae254a3a0a0b56fffc45d9a261bd1f1dd631d74a3cf1dc5ad05aac40df703aba30cdb716ea0abb4068f04affd23a397c999c56fc1f9117a9422f162552e93a7d26abc3bd345a094ef7f00e9f21b4b691be4f1795becb54049b37828e100e9c67b9e38e6ae09e08080a0df80f9bde4c8c04f7c6757438f1acffa0affb75d48b5e85049c25dfac10dc4ff80",
                "0xf86c9d333b108031527c38b3550879600b92d4b747494ed4af1b3af6ffdbcf78b84cf84a018633d758c09000a056e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421a0c5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470"
            ]);
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_get_proof_for_previous_fork_block_works_even_with_genesis_data_on_fork(
    ) -> anyhow::Result<()> {
        let provider = setup_provider(MinimalProviderConfig::fork_with_accounts(ForkConfig {
            block_number: Some(FORK_BLOCK_NUMBER),
            cache_dir: edr_defaults::CACHE_DIR.into(),
            chain_overrides: HashMap::default(),
            http_headers: None,
            url: json_rpc_url_provider::ethereum_mainnet(),
        }))?;
        verify_account_proof(&provider, ADDRESS, BlockSpec::Number(FORK_BLOCK_NUMBER - 1),
            // proof obtained from Alchemy eth_getProof call
            [
                "0xf90211a0d1350b292ec4abf1f85464a3fb229d26b44a34a587339071d031fafc0eeca724a036157ecb79e7748cdfbd1d41f753fb902e902c80ad32bf81bd1ed8ab0ac949b2a00e86be3f1c4125b4831bb791c05c17d863eeb8edf4272e6c68e62557d12f750ba01687e6f3faa6151b4e25740896c83cbe440db3070c320eeae609a08f5c6e5a2da017579b44dae1b1e41282142ef40bc585b8569cc12ca914b4e7f43bee9209be7da0854a9016096ce65023e31bd918dc7ff64a65725091201b41c8f4ae0ddd7c045aa061d0f17b9e30f6bb21be98175247068e3f992609cf8571d1c5293931550c925ea00e26ced4deb44a472ae23127d62e026b147e2cacc0965d63af2b165ac2b28588a0b35c6e8f963e3a095710f546b8a5b710a7a5e85fea733b0c0ba91a18e8dacb7da07e3ce64169e7606b6251ec977a3d6d787c4add21c00fa707f7621958ea6093b8a018ee6c3b58972e8433e4da7906ecf44f158c21889a9002f640f9a96f3c1ae6dda070db91499e4cd691ad323f9d1401669161ff173365e194b7aac8fbb9de26e659a000e541e369c482a96fddafbea9b4672fca04ee04ca6d1b705662e601afcaae0aa0a5d1b411474b6f9788a0e4292d8ea50567658997defe238dd56196863283409ea039d25295afb3836b5e184f99d161340b2c350abca16e1e0c17f8d2106c776b77a0cd9e4de5234e4d7140515627f7d14078b01d6471e2fbc43abd478fb4a75fde6180",
                "0xf90211a099f176f4800bf10cf34b9078fabd7ee15d3a057804f7c2f32da5db96ed429b63a0eb5395967447203a384e35bcb14cea3f8edb1782a6eb8718c966f4b1931f8b37a09bafe9f7ed754b959b47134f5ea364abf9ec51e7f1d28f569653a2ef6722f5a1a0e8f4d8832a1001913b50c1852c5134881f4c616a9e3ab2c1ca56e01d9d74616ba066c967ef7a962f7c056cdbd3878651975303a3c6a56ef7695f0a90b675c4723ba0d9214cae23b1556e9817bd6588dea8993679b92fdc5fda95f5a35e3e98d30081a0c2503727907c2f2660ec8b3953aeb400823ae292a1fdfeb7ff041c22486a6f4da0a80ba3c9faf0f043f000b8c8e8b3b38bb2f24bfb4183e8dc55df77443e98da81a06df2bdcbfebcc2812165fab168f00cca054533791e1a4f2ff21217112bfc6ab6a089651e6f20d1812c3468673d74c1644775ffd4a59f267abf1de31932fbd935caa0e24a026f9004aab0ee0ff94f56754a44c835c3675b67fb29bf1e56f908ac2be5a0dfbd565bd467df94fe2001f1a2148bdfcee1df078e54170fd5b5d0dbc4a28964a0d8528e522e26ab030a72dd41979ebc5881efb127e3313ffce8d79c4b88a410d7a0f1d2ba035698070610f95a3bb7c7cc8c9bf5002e7ffcae2f8a3f3e7f4dcbdd4ba06dcba44018ea02167efc53731fc6e705a774da0f668f43e2fc2006baf6633a85a001d08ee5260b21559b992a9f63c1786476bf353a73288244b60cd9c5fb4851a680",
                "0xf90211a099c5e88990c83fe16b39e0f74734a5e13b26c76fc636b0b4a4d41ff550489108a066d1d7156f1b5117030df1ca31d4eac6b79f39739368eebe742f9b1211ec3bc4a0a8a5f02809c5860ee9bf9a3145cd5e129052a8f20867ab6ba5c1e59240e858e4a06e59621254f1c11ce5f984d6ec3442dbf8834f710044a3049d3baa1d32df59e3a019aedc36559df6d4d7dde824af879d596fb08cf794131f17191773eb2a7a0516a052ab65a6ca4db9fbcb4f1d45f001d6e0bcceb3d892a4e86b3e05768e6470dabca0ea2c30606aa56f63f4e4effe04e8791a6642f9507257cfa31a3288f17f263511a05e3ddd097ecd2a53d92e29d1c969a6dd0f688807b0879bb8d56121a370d2aef5a0d7b2e0ee1048587843aeb3ee125a2d2d5ae4133c5d55c51dc8cb9a99a7e2df8fa0594786a979024f0aed519be02fb47ab5ac2c890cb48fc8ea83fb42a29165ee53a068224651925bc9976af03a34a4a575ea96867cc955189257bc291d3f78a5cea4a08ce4659712ec0162e9c118dd88e465c5b41998321c38e550e22671a6af05a56ba0c44884774a0b3c8e5fa15388555c50a484a0d24ed32d16f0d5c59a37bd3acf02a054a562fc1d29ee5a0ce5dc049d49dd941c719a5c19cd78630e43c53ba420cbd1a007d60c6f82eccba737a562ebc976463646ca76e32407174839c428241b67ddc1a0c81fc28b628ac12f4d16db7ea97290776eccd9de66ce4c4a06bd9c355df5c89380",
                "0xf90211a06c7ed6fae37b3fa435dbc6e9d967f0794c7b2216e4856ae22b51aab41b0463bda02d30c29571aaedafb510944553b3c7ca5044ee656217bfb677271c2e88d93950a0e8bce5430da5365d9dca1ffaca3013162f1aa1a7274e389f224cdeb3d96a51b5a0debb16611dda2f29a482e24a1da0ff9cfedde99f0c10a271077789ff567faf80a0a6be18b3bab1e7ad7d68c61a49f48f8355bbd29fd92c000ecc24d54f509ed45aa02894f11168e37ccf25d8b2f52b6623eb4071427498c9b3954688afeba3af70c8a0aba0881c72a5eb7508d82368d1297889056075d2b591d6b2b75c2934b44f7658a01570b7f43da943dc5f2415651c46cf5e1c2744839568d4ddde8e47dd546f2a7fa07a04e4b021c1e60fab0ae99393821844151786ead13019a7b8d6cdd1ebc04347a0efe5b197a93961b40c00001f3346e1db4bfb4b02aa1d0d2ed963c1843634997aa0d1716c20dee8d9e69b8a1dae0503854d4c2b714ee00e4dddc7a587159fcbec2ca04f66e806c16a6dc80a4cf50fa767a0a48f97d3068e9289da9706299fc006d3b1a00da999ef45b3ff25b58b6651a5de30d11df8123fe5ad0dad40a101bd56235281a0f189f6119bd2b1eff1f5da17938fda38bf8a512a4bededf41b014a83c79bfc79a09e69e819fdace598a77a15432f73145b5c5063a03bb528459095f049902e99b0a03acdc54794cfef6c48a25611623fcc404fbdffec246ba83f312907caf825933780",
                "0xf90211a0d3a4695731a5270ab112a9218056825cff4872355844048535d1eb73176b93c6a00b0a1c375fb506442349ce25db5d65057f3162ff22a873a6aebf4e479bf1e18aa0afb11a37d5deb61e339f3fae41c4cee26f9e211bbed046238772169100c441e0a0c28d4037ff24332a1a05f84d6a74d1521677b60dbb38a081f7e1af273ceee83ea0cb943cf12ae1959e93543f264481b22db2cdcfc25d355fcce5bdef6bb791fbc7a0ec69e0f4f941d10f3001bfacc4cc56e589bfcc9e05d8692c9a4185c2415ef26ca04b74296be3f998cdf881f797254e3d61f3f15f87b330c2e627d5ca96af8e482ea013bfbbc59b0fe5e77dfcdd3c79b519d7d2cea24daa486dbedba2cd6ef5c1d390a0dcc148f10d697cd4d26dca579232580f21053473da940c5871ea4d4d71c322cda03f0fd9d194b5b7413db6ed6d567bde73b1daa4abbd82277ab751c6b3f1343f95a0f2d2ae6c43839dcbf3b81b92549fbfec15306e39090e5b17f71ea2cb357106c6a0cea753ad7124f8b208bf4804a6ea9d417d4ecd91e91d202dea824fda2edc6569a0d8004eae1cf19bf2d5b218a769ed4fcb5b8207169b1d7374c0ff3c1ff6248724a0ff3895d286c7f3e4ec6a1303e44e82e9f288aad423c8c6ffcaf2f1f582e9e357a06d3f3b31e065c633ddddb4031dac5a74a8fd8f1493df7e065d72e67d0a1ba65fa0e11cdccc61274056c39e7ac188bbc6c484a34fd8a75fc69330b2831038aea55a80",
                "0xf90211a06da132f7e64a41a50a276e017068bb30be53e17fa77d5ebbb721e7f0c37681a4a0a92d4ee08606231968b3202daab8bc7179ad5c2ad4f4f0e7c57d4cf8e8d1b87ea0d895e96c0d389ec4997980e7dfc0b2d59427dd3af15c31f25bad841d41487847a06c8cee1c2dd39829b6a97e29d5415a71d94dbb63d80095f3c7b2defd61abdcc5a0c4c1bf755cedee90d63adf3cd4fd9492931a02e887d44103f163e1aa58a9cc36a0a293402820a12198ae3dd3b9a71d02c43cc39a59089dba51ffffebe36a909431a0fc44db787f4078d72e7f5de5f3100993fd869f9037415cb37ed13e60f25ad890a0c1cdbdaa6a000f632e960a394ed96ad62cb8a27f4bb62ae21cc782c0af12737aa0b8497ed6b3507da6a4292e6201d74ad290319589001d4418b2055271a9a6e92fa0095aad802ba56641f6db8729d18aed9e8baacb39cbb82d50661cf05a76b80470a07bf8deee4fd5d2fb2ed2ad6a99b0498edc3813fd740c49329f7056698814935da0e86604d6f5d372a69d71508028162cbac71a5693b9791a0394a7e1ba43a6ee28a04ce5cc541e6e201edf7825ac3ec09b270e66d0d855c9b387bea03a1996a4b32aa0d6581d89bcdedcc19cca320dca0e3395640a6f24f9ce262de1b8671a7fb43842a00012151af46985a9e583e9553f8f49bc24bd33a1df6a0db5edf6118e158fd73aa0bbc79c4b5d93439c3c61ffd0b1a703d9eb7dfa552f4bf2ae14c048bdc750ba9880",
                "0xf901d1a095506fef4e979268f00ea8dbb5b4617c0157ae4de2efcbf15469be5cf4597846a08eddf70d4dc30906b3da63ca9c0866f9118250eef5a7f9b2ef6e8345d8748ea6a04fec185b10c60d84d79f9e17c7e334dfa3c82432a4c85622f2f5b955bb665b91a04f5a306c13214c119f44fc3b45c1db1177cd11418a0bfc454342ddb6b30d70f4a009ae290d6c16fdff4ca4fbfbc4e6c7e61f53f450ad131e2b9b93acb79e87f195a0a62bc61a2f079ec4143d423feecd80eb5e297556d6dc016091da262686f03553a07952347da7107711abc18be43ef96aaa07a1ebaaae88fa375652272437615ee1a0773aa3fb602c21b5825a66b069a252c36ed86056fa841249234ef1869cc8c271a07e58d5e199df1e237781f7b14fa5bd5a31ec4f52b62e7bdbea3211ecf3ee2637a05dc2e5292b687c0d1f41f73d040079203c956af923ec4881a237aa016ae254a3a0a0b56fffc45d9a261bd1f1dd631d74a3cf1dc5ad05aac40df703aba30cdb716ea0abb4068f04affd23a397c999c56fc1f9117a9422f162552e93a7d26abc3bd345a094ef7f00e9f21b4b691be4f1795becb54049b37828e100e9c67b9e38e6ae09e08080a0df80f9bde4c8c04f7c6757438f1acffa0affb75d48b5e85049c25dfac10dc4ff80",
                "0xf86c9d333b108031527c38b3550879600b92d4b747494ed4af1b3af6ffdbcf78b84cf84a018633d758c09000a056e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421a0c5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470"
            ]);
        Ok(())
    }
}
