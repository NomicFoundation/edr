//! Replays a case's transactions through the provider.

use std::{collections::HashMap, num::NonZeroU64, sync::Arc};

use alloy_dyn_abi::{DynSolType, DynSolValue, Specifier as _};
use alloy_json_abi::{JsonAbi, Param};
use alloy_primitives::I256;
use anyhow::{bail, ensure, Context as _};
use edr_chain_l1::{
    rpc::{BlockReceipt, TransactionRequest},
    L1ChainSpec, L1Hardfork, L1SignedTransaction,
};
use edr_primitives::{hex, Address, Bytes, B256, U256};
use edr_provider::{
    config::AccountOverride,
    observability::EvmObservedData,
    test_utils::{create_test_config_with, MinimalProviderConfig},
    time::CurrentTime,
    Logger, MethodInvocation, MineBlockResultWithMetadataForChainSpec, Provider, ProviderError,
    ProviderErrorForChainSpec, ProviderRequest, TransactionFailureReason,
};
use edr_signer::public_key_to_address;
use edr_solidity::{
    contract_decoder::ContractDecoder,
    library_utils::link_hex_string_bytecode,
    solidity_stack_trace::{StackTraceCreationResult, StackTraceEntry},
};
use parking_lot::{Mutex, RwLock};
use serde_json::Value;

use crate::{
    compare,
    compiler::Compilation,
    console_log,
    corpus::{Case, Transaction},
    CompilerConfig,
};

const SENDER_PRIVATE_KEY: &str = "e331b6d69882b4cb4ea581d88e0b604039a3de5967688d3dcffdd2270c0fd109";
const DEFAULT_GAS: u64 = 4_000_000;
const GAS_PRICE: u128 = 10;

struct Deployed<'a> {
    file: &'a str,
    contract: &'a str,
    address: Address,
}

pub(crate) fn run(
    case: &Case,
    compilation: &Compilation,
    config: &CompilerConfig,
    runtime: &tokio::runtime::Handle,
) -> anyhow::Result<()> {
    let console_logs = Arc::new(Mutex::new(Vec::new()));
    let (provider, sender) = make_provider(compilation, console_logs.clone(), runtime)?;
    let mut deployed: HashMap<usize, Deployed<'_>> = HashMap::new();

    for (index, transaction) in case.definition.transactions.iter().enumerate() {
        let result = if let Some(file) = &transaction.file {
            let contract = transaction
                .contract
                .as_deref()
                .context("deployment without a contract")?;
            let data = deployment_data(compilation, file, contract, transaction, &deployed)
                .with_context(|| format!("tx {index}: building the deployment"))?;
            let outcome = send(&provider, sender, None, data, transaction)?;
            if let Outcome::Succeeded { contract_address } = &outcome {
                let address = contract_address
                    .with_context(|| format!("tx {index}: deployment created no contract"))?;
                deployed.insert(
                    index,
                    Deployed {
                        file,
                        contract,
                        address,
                    },
                );
            }
            outcome
        } else {
            let target = transaction
                .to
                .and_then(|to| deployed.get(&to))
                .with_context(|| {
                    format!("tx {index}: calls a transaction that deployed nothing")
                })?;
            let data = call_data(compilation, target, transaction)
                .with_context(|| format!("tx {index}: building the calldata"))?;
            send(&provider, sender, Some(target.address), data, transaction)?
        };

        match (result, &transaction.stack_trace) {
            (Outcome::Succeeded { .. }, None) => {}
            (Outcome::Succeeded { .. }, Some(_)) => bail!("tx {index} should have failed"),
            (Outcome::Failed(failure), None) => {
                bail!("tx {index} shouldn't have failed: {failure}")
            }
            (Outcome::Failed(Failure::Trace(trace)), Some(expected)) => {
                compare::stack_traces(&trace, expected, config)
                    .with_context(|| format!("tx {index} stack trace"))?;
            }
            (Outcome::Failed(failure), Some(_)) => bail!("tx {index}: {failure}"),
        }

        if let Some(expected) = &transaction.console_logs {
            let actual = console_logs
                .lock()
                .iter()
                .map(console_log::decode)
                .collect::<anyhow::Result<Vec<_>>>()?;
            let expected: Vec<Vec<String>> = expected
                .iter()
                .map(|arguments| arguments.iter().map(value_to_string).collect())
                .collect();
            ensure!(
                actual == expected,
                "tx {index} console logs {actual:?}, expected {expected:?}"
            );
        }
    }
    Ok(())
}

enum Outcome {
    Succeeded { contract_address: Option<Address> },
    Failed(Failure),
}

enum Failure {
    Trace(Vec<StackTraceEntry>),
    /// What napi reports as no stack trace, or as a failed inference.
    NoTrace(String),
}

impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Failure::Trace(trace) => write!(f, "stack trace of {} entries", trace.len()),
            Failure::NoTrace(reason) => write!(f, "{reason}"),
        }
    }
}

fn send(
    provider: &Provider<L1ChainSpec>,
    from: Address,
    to: Option<Address>,
    data: Bytes,
    transaction: &Transaction,
) -> anyhow::Result<Outcome> {
    let request = TransactionRequest {
        from,
        to,
        data: Some(data),
        value: transaction.value.as_ref().map(parse_u256).transpose()?,
        gas: Some(
            transaction
                .gas
                .as_ref()
                .map(|gas| parse_u256(gas).map(|gas| gas.to::<u64>()))
                .transpose()?
                .unwrap_or(DEFAULT_GAS),
        ),
        gas_price: Some(GAS_PRICE),
        ..TransactionRequest::default()
    };

    match provider.handle_request(ProviderRequest::with_single(
        MethodInvocation::SendTransaction(request),
    )) {
        Ok(response) => {
            let hash: B256 = response.deserialize_result()?;
            let receipt = provider.handle_request(ProviderRequest::with_single(
                MethodInvocation::GetTransactionReceipt(hash),
            ))?;
            let receipt: BlockReceipt = receipt.deserialize_result()?;
            Ok(Outcome::Succeeded {
                contract_address: receipt.contract_address,
            })
        }
        Err(error) => Ok(Outcome::Failed(failure(error)?)),
    }
}

fn failure(error: ProviderErrorForChainSpec<L1ChainSpec>) -> anyhow::Result<Failure> {
    let ProviderError::TransactionFailed(failure) = error else {
        bail!("unexpected provider error: {error}");
    };
    // Mirrors napi, which reports no stack trace for a top-level out-of-gas.
    if matches!(
        failure.failure.reason,
        TransactionFailureReason::OutOfGas(_)
    ) {
        return Ok(Failure::NoTrace(
            "out of gas, without a stack trace".to_owned(),
        ));
    }
    Ok(match &failure.failure.stack_trace_result {
        StackTraceCreationResult::Success(trace) => Failure::Trace(trace.clone()),
        StackTraceCreationResult::HeuristicFailed => {
            Failure::NoTrace("stack trace heuristics failed".to_owned())
        }
        StackTraceCreationResult::Error(error) => {
            Failure::NoTrace(format!("stack trace creation failed: {error}"))
        }
    })
}

fn make_provider(
    compilation: &Compilation,
    console_logs: Arc<Mutex<Vec<Bytes>>>,
    runtime: &tokio::runtime::Handle,
) -> anyhow::Result<(Provider<L1ChainSpec>, Address)> {
    let secret_key = k256::SecretKey::from_slice(&hex::decode(SENDER_PRIVATE_KEY)?)?;
    let sender = public_key_to_address(secret_key.public_key());

    let mut config = create_test_config_with(MinimalProviderConfig::local_empty());
    config.hardfork = L1Hardfork::Cancun;
    config.chain_id = 1;
    config.network_id = 1;
    config.mining.block_gas_limit = NonZeroU64::new(10_000_000);
    config.min_gas_price = 0;
    config.initial_base_fee_per_gas = Some(0);
    config.coinbase = Address::ZERO;
    config.bail_on_transaction_failure = true;
    config.bail_on_call_failure = false;
    config.owned_accounts = vec![secret_key];
    config.genesis_state.insert(
        sender,
        AccountOverride {
            balance: Some(U256::from(10u64.pow(15))),
            ..AccountOverride::default()
        },
    );

    let decoder = ContractDecoder::new(compilation.build_info.clone());
    let provider = Provider::new(
        runtime.clone(),
        Box::new(ConsoleLogCapture { console_logs }),
        Box::new(|_| {}),
        config,
        Arc::new(RwLock::new(decoder)),
        CurrentTime,
    )?;
    Ok((provider, sender))
}

/// Collects the encoded `console.log` calls of every sent transaction.
#[derive(Clone)]
struct ConsoleLogCapture {
    console_logs: Arc<Mutex<Vec<Bytes>>>,
}

impl Logger<L1ChainSpec, CurrentTime> for ConsoleLogCapture {
    fn is_enabled(&self) -> bool {
        false
    }

    fn set_is_enabled(&mut self, _is_enabled: bool) {}

    fn log_send_transaction(
        &mut self,
        _transaction: &L1SignedTransaction,
        mining_results: &[MineBlockResultWithMetadataForChainSpec<L1ChainSpec, EvmObservedData>],
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut console_logs = self.console_logs.lock();
        for observed in mining_results
            .iter()
            .flat_map(|result| &result.transaction_inspector_data)
        {
            console_logs.extend(observed.encoded_console_logs.iter().cloned());
        }
        Ok(())
    }

    fn print_method_logs(
        &mut self,
        _method: &str,
        _error: Option<&ProviderErrorForChainSpec<L1ChainSpec>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }
}

fn contract_output<'a>(
    compilation: &'a Compilation,
    file: &str,
    contract: &str,
) -> anyhow::Result<&'a Value> {
    compilation
        .output
        .get("contracts")
        .and_then(|contracts| contracts.get(file))
        .and_then(|contracts| contracts.get(contract))
        .with_context(|| format!("{file}:{contract} not in the compiler output"))
}

fn abi(contract: &Value) -> anyhow::Result<JsonAbi> {
    let abi = contract.get("abi").context("contract without an abi")?;
    Ok(serde_json::from_value(abi.clone())?)
}

fn deployment_data(
    compilation: &Compilation,
    file: &str,
    contract: &str,
    transaction: &Transaction,
    deployed: &HashMap<usize, Deployed<'_>>,
) -> anyhow::Result<Bytes> {
    let output = contract_output(compilation, file, contract)?;
    let bytecode = output
        .pointer("/evm/bytecode")
        .context("contract without bytecode")?;
    let mut code = bytecode
        .get("object")
        .and_then(Value::as_str)
        .context("bytecode without an object")?
        .to_owned();

    let link_references = bytecode
        .get("linkReferences")
        .and_then(Value::as_object)
        .into_iter()
        .flatten();
    for (library_file, libraries) in link_references {
        for (library, references) in libraries.as_object().into_iter().flatten() {
            let address = transaction
                .libraries
                .get(library_file)
                .and_then(|libraries| libraries.get(library))
                .and_then(|index| deployed.get(index))
                .with_context(|| format!("library {library_file}:{library} not deployed"))?
                .address;
            for reference in references.as_array().into_iter().flatten() {
                let start = reference
                    .get("start")
                    .and_then(Value::as_u64)
                    .context("link reference without a start")?;
                code = link_hex_string_bytecode(code, &hex::encode(address), start.try_into()?)?;
            }
        }
    }
    ensure!(!code.contains('_'), "unlinked libraries remain");

    let mut data = hex::decode(&code)?;
    let abi = abi(output)?;
    if let Some(constructor) = &abi.constructor
        && !transaction.params.is_empty()
    {
        data.extend(encode_params(&constructor.inputs, &transaction.params)?);
    }
    Ok(data.into())
}

fn call_data(
    compilation: &Compilation,
    target: &Deployed<'_>,
    transaction: &Transaction,
) -> anyhow::Result<Bytes> {
    if let Some(data) = &transaction.data {
        return Ok(hex::decode(data)?.into());
    }
    let Some(name) = &transaction.function else {
        return Ok(Bytes::new());
    };

    let abi = abi(contract_output(compilation, target.file, target.contract)?)?;
    let function = abi
        .functions()
        .find(|function| {
            &function.name == name && function.inputs.len() == transaction.params.len()
        })
        .with_context(|| {
            format!(
                "no function {name} taking {} params",
                transaction.params.len()
            )
        })?;
    let mut data = function.selector().to_vec();
    data.extend(encode_params(&function.inputs, &transaction.params)?);
    Ok(data.into())
}

fn encode_params(inputs: &[Param], params: &[Value]) -> anyhow::Result<Vec<u8>> {
    let values = inputs
        .iter()
        .zip(params)
        .map(|(input, param)| to_dyn_value(&input.resolve()?, param))
        .collect::<anyhow::Result<Vec<_>>>()?;
    Ok(DynSolValue::Tuple(values).abi_encode_params())
}

fn to_dyn_value(ty: &DynSolType, value: &Value) -> anyhow::Result<DynSolValue> {
    Ok(match (ty, value) {
        (DynSolType::Array(inner), Value::Array(items)) => DynSolValue::Array(
            items
                .iter()
                .map(|item| to_dyn_value(inner, item))
                .collect::<anyhow::Result<_>>()?,
        ),
        (DynSolType::FixedArray(inner, _), Value::Array(items)) => DynSolValue::FixedArray(
            items
                .iter()
                .map(|item| to_dyn_value(inner, item))
                .collect::<anyhow::Result<_>>()?,
        ),
        // ethereumjs-abi encodes a hex `int` as a raw two's-complement word.
        (DynSolType::Int(bits), Value::String(value)) if value.starts_with("0x") => {
            DynSolValue::Int(I256::from_raw(value.parse()?), *bits)
        }
        (DynSolType::Bytes | DynSolType::FixedBytes(_), Value::Array(items)) => {
            let bytes = items
                .iter()
                .map(|item| {
                    item.as_u64()
                        .and_then(|byte| u8::try_from(byte).ok())
                        .context("bytes given as an array of non-byte values")
                })
                .collect::<anyhow::Result<Vec<u8>>>()?;
            ty.coerce_str(&format!("0x{}", hex::encode(bytes)))?
        }
        (_, Value::Bool(value)) => DynSolValue::Bool(*value),
        // ethereumjs-abi left-pads odd-length hex.
        (_, Value::String(value)) if value.starts_with("0x") && value.len() % 2 == 1 => {
            ty.coerce_str(&format!("0x0{}", &value[2..]))?
        }
        (_, Value::String(value)) => ty.coerce_str(value)?,
        (_, Value::Number(value)) => ty.coerce_str(&value.to_string())?,
        _ => bail!("cannot encode {value} as {ty}"),
    })
}

/// Parses a decimal or `0x` hex quantity, as a JSON number or string.
pub(crate) fn parse_u256(value: &Value) -> anyhow::Result<U256> {
    let text = match value {
        Value::Number(number) => number.to_string(),
        Value::String(text) => text.clone(),
        other => bail!("not a quantity: {other}"),
    };
    Ok(text.parse()?)
}

fn value_to_string(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}
