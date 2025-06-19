use std::sync::{Mutex, OnceLock};

use edr_eth::l1::HaltReason;
use edr_solidity::{
    artifacts::{ArtifactId, BuildInfoConfigWithBuffers},
    contract_decoder::{ContractDecoder, ContractDecoderError, NestedTraceDecoder},
    linker::{LinkOutput, Linker},
    nested_trace::NestedTrace,
};
use edr_solidity_tests::{
    constants::LIBRARY_DEPLOYER,
    contracts::ContractsByArtifact,
    decode::RevertDecoder,
    evm_context::L1EvmBuilder,
    multi_runner::{TestContract, TestContracts},
    revm::context::TxEnv,
    MultiContractRunner, SolidityTestRunnerConfig,
};
use foundry_compilers::artifacts::Libraries;

use crate::{
    provider::TracingConfigWithBuffers,
    solidity_tests::{
        artifact::{Artifact as JsArtifact, ArtifactId as JsArtifactId},
        config::SolidityTestRunnerConfigArgs,
    },
};

pub(super) async fn build_runner(
    artifacts: Vec<JsArtifact>,
    test_suites: Vec<JsArtifactId>,
    config_args: SolidityTestRunnerConfigArgs,
    tracing_config: TracingConfigWithBuffers,
) -> napi::Result<
    MultiContractRunner<
        edr_eth::l1::BlockEnv,
        (),
        L1EvmBuilder,
        edr_eth::l1::HaltReason,
        edr_eth::l1::SpecId,
        LazyContractDecoder,
        TxEnv,
    >,
> {
    let config = SolidityTestRunnerConfig::try_from(config_args)?;

    let artifact_contracts = artifacts
        .into_iter()
        .map(|artifact| Ok((artifact.id.try_into()?, artifact.contract.try_into()?)))
        .collect::<napi::Result<Vec<_>>>()?;
    let linker = Linker::new(config.project_root.clone(), artifact_contracts);
    let LinkOutput {
        libraries,
        libs_to_deploy,
    } = linker
        .link_with_nonce_or_address(
            Libraries::default(),
            LIBRARY_DEPLOYER,
            0,
            linker.contracts.keys(),
        )
        .map_err(|err| napi::Error::from_reason(err.to_string()))?;
    let linked_contracts = linker
        .get_linked_artifacts(&libraries)
        .map_err(|err| napi::Error::from_reason(err.to_string()))?;

    let known_contracts = ContractsByArtifact::new(linked_contracts);

    let test_suites = test_suites
        .into_iter()
        .map(TryInto::try_into)
        .collect::<Result<Vec<ArtifactId>, _>>()?;

    let contract_decoder = LazyContractDecoder::new(tracing_config);

    // Build revert decoder from ABIs of all artifacts.
    let abis = known_contracts.iter().map(|(_, contract)| &contract.abi);
    let revert_decoder = RevertDecoder::new().with_abis(abis);

    let contracts = test_suites
        .iter()
        .map(|artifact_id| {
            let contract_data = known_contracts.get(artifact_id).ok_or_else(|| {
                napi::Error::new(
                    napi::Status::GenericFailure,
                    format!("Unknown contract: {}", artifact_id.identifier()),
                )
            })?;

            let bytecode = contract_data.bytecode.clone().ok_or_else(|| {
                napi::Error::new(
                    napi::Status::GenericFailure,
                    format!(
                        "No bytecode for test suite contract: {}",
                        artifact_id.identifier()
                    ),
                )
            })?;

            let test_contract = TestContract {
                abi: contract_data.abi.clone(),
                bytecode,
            };

            Ok((artifact_id.clone(), test_contract))
        })
        .collect::<napi::Result<TestContracts>>()?;

    MultiContractRunner::new(
        config,
        contracts,
        known_contracts,
        libs_to_deploy,
        contract_decoder,
        revert_decoder,
    )
    .await
    .map_err(|err| {
        napi::Error::new(
            napi::Status::GenericFailure,
            format!("Failed to create multi contract runner: {err}"),
        )
    })
}

/// Only parses the tracing config which is very expensive if the contract
/// decoder is used.
#[derive(Debug)]
pub struct LazyContractDecoder {
    // We need the `Mutex`, because `Uint8Array` is not `Sync`
    tracing_config: Mutex<TracingConfigWithBuffers>,
    // Storing the result so that we can propagate the error
    contract_decoder: OnceLock<Result<ContractDecoder, ContractDecoderError>>,
}

impl LazyContractDecoder {
    fn new(tracing_config: TracingConfigWithBuffers) -> Self {
        Self {
            tracing_config: Mutex::new(tracing_config),
            contract_decoder: OnceLock::new(),
        }
    }
}

impl NestedTraceDecoder<HaltReason> for LazyContractDecoder {
    fn try_to_decode_nested_trace(
        &self,
        nested_trace: NestedTrace<HaltReason>,
    ) -> Result<NestedTrace<HaltReason>, ContractDecoderError> {
        self.contract_decoder
            .get_or_init(|| {
                let tracing_config = self
                    .tracing_config
                    .lock()
                    .expect("Can't get poisoned, because only called once");
                edr_solidity::artifacts::BuildInfoConfig::parse_from_buffers(
                    BuildInfoConfigWithBuffers::from(&*tracing_config),
                )
                .map_err(|err| ContractDecoderError::Initialization(err.to_string()))
                .and_then(|config| ContractDecoder::new(&config))
            })
            .as_ref()
            .map_err(Clone::clone)
            .and_then(|decoder| decoder.try_to_decode_nested_trace(nested_trace))
    }
}
