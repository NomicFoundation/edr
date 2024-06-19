/// Based on `crates/foundry/forge/tests/it/test_helpers.rs`.
use std::{path::PathBuf, sync::Arc};

use alloy_primitives::U256;
use forge::{
    constants::CALLER,
    decode::RevertDecoder,
    multi_runner::TestContract,
    opts::{Env as EvmEnv, EvmOpts},
    revm::primitives::SpecId,
    MultiContractRunner, MultiContractRunnerBuilder, TestOptionsBuilder,
};
use foundry_compilers::ArtifactId;
use foundry_config::{Config, RpcEndpoint, RpcEndpoints};

pub(super) fn build_runner(
    test_suites: Vec<(ArtifactId, TestContract)>,
    gas_report: bool,
) -> napi::Result<MultiContractRunner> {
    let config = foundry_config(gas_report);

    let mut evm_opts = evm_opts();
    evm_opts.isolate = config.isolate;
    evm_opts.verbosity = config.verbosity;
    evm_opts.memory_limit = config.memory_limit;
    evm_opts.env.gas_limit = config.gas_limit.into();

    let test_options = TestOptionsBuilder::default()
        .fuzz(config.fuzz.clone())
        .invariant(config.invariant.clone())
        .build_hardhat()
        .expect("Config loaded");

    let builder = MultiContractRunnerBuilder::new(Arc::new(config))
        .sender(evm_opts.sender)
        .with_test_options(test_options);

    let abis = test_suites.iter().map(|(_, contract)| &contract.abi);
    let revert_decoder = RevertDecoder::new().with_abis(abis);

    let evm_env = evm_opts.local_evm_env();

    Ok(MultiContractRunner {
        contracts: test_suites.into_iter().collect(),
        evm_opts,
        env: evm_env,
        evm_spec: builder.evm_spec.unwrap_or(SpecId::MERGE),
        sender: builder.sender,
        revert_decoder,
        fork: builder.fork,
        config: builder.config,
        coverage: builder.coverage,
        debug: builder.debug,
        test_options: builder.test_options.unwrap_or_default(),
        isolation: builder.isolation,
        output: None,
    })
}

fn project_root() -> PathBuf {
    PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../crates/foundry/testdata"
    ))
}

fn foundry_config(gas_report: bool) -> Config {
    const TEST_PROFILE: &str = "default";

    // Forge project root.
    let root = project_root();

    let mut config = Config::with_root(&root);

    config.ast = true;
    config.src = root.join(TEST_PROFILE);
    config.out = root.join("out").join(TEST_PROFILE);
    config.cache_path = root.join("cache").join(TEST_PROFILE);
    config.libraries =
        vec!["fork/Fork.t.sol:DssExecLib:0xfD88CeE74f7D78697775aBDAE53f9Da1559728E4".to_string()];

    config.rpc_endpoints = rpc_endpoints();
    // TODO https://github.com/NomicFoundation/edr/issues/487
    // config.allow_paths.push(manifest_root().to_path_buf());

    // no prompt testing
    config.prompt_timeout = 0;

    if !gas_report {
        config.fuzz.gas_report_samples = 0;
        config.invariant.gas_report_samples = 0;
    }

    config
}

fn evm_opts() -> EvmOpts {
    EvmOpts {
        env: EvmEnv {
            gas_limit: u64::MAX,
            chain_id: None,
            tx_origin: CALLER,
            block_number: 1,
            block_timestamp: 1,
            ..Default::default()
        },
        sender: CALLER,
        initial_balance: U256::MAX,
        ffi: true,
        verbosity: 3,
        memory_limit: 1 << 26,
        ..Default::default()
    }
}

/// The RPC endpoints used during tests.
fn rpc_endpoints() -> RpcEndpoints {
    RpcEndpoints::new([("alchemy", RpcEndpoint::Url("${ALCHEMY_URL}".to_string()))])
}
