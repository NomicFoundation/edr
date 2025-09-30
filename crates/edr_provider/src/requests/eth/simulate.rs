use edr_eth::{
    block::{self, HeaderOverrides},
    filter::LogOutput,
    result::ExecutionResult,
    BlockSpec, Bytes, B256,
};
use edr_evm::{
    spec::RuntimeSpec, state::StateOverrides, transaction, Block, MineBlockResultAndState,
};
use edr_evm_spec::{EvmTransactionValidationError, TransactionValidation};
use edr_rpc_eth::{
    simulate::{SimBlock, SimulatePayload},
    BlockOverrides,
};
use edr_signer::FakeSign;

use crate::{
    data::ProviderData,
    error::ProviderErrorForChainSpec,
    requests::eth::{block_to_rpc_output, HashOrTransaction},
    spec::{FromRpcType, Sender, SyncProviderSpec, TransactionContext},
    time::TimeSinceEpoch,
    ProviderError,
};

const MAX_SIMULATE_BLOCKS: usize = 256;
// TODO: do we check this?
const MAX_WITHDRAWALS: usize = 16;
// TODO: does this depend on the chain?
const TIMESTAMP_INCREMENT: u64 = 12;

// TODO: is this ok? needed for serde::Deserialize
impl<RpcTransaction> Default for HashOrTransaction<RpcTransaction> {
    fn default() -> Self {
        Self::Hash(B256::default())
    }
}
#[derive(serde::Serialize, serde::Deserialize)]
pub struct SimResult<RpcTransaction> {
    pub block: edr_rpc_eth::Block<HashOrTransaction<RpcTransaction>>,
    pub calls: Vec<SimCallResult>,
}
#[derive(serde::Serialize, serde::Deserialize)]
pub struct SimError {
    // write error codes
    pub code: i32,
    pub message: String,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct SimCallResult {
    pub status: bool,
    pub return_data: Bytes,
    pub gas_used: u64,
    pub logs: Vec<LogOutput>,
    pub error: Option<SimError>,
}

// TODO: move some functionality to data.rs to avoid making functions public
pub fn handle_simulatev1_request<
    ChainSpecT: SyncProviderSpec<
        TimerT,
        BlockEnv: Default,
        SignedTransaction: Clone
                               + Default
                               + TransactionValidation<
            ValidationError: From<EvmTransactionValidationError> + PartialEq,
        >,
    >,
    TimerT: Clone + TimeSinceEpoch,
>(
    data: &mut ProviderData<ChainSpecT, TimerT>,
    simulate_payload: SimulatePayload<ChainSpecT::RpcTransactionRequest>,
    block_spec: Option<BlockSpec>,
) -> Result<Vec<SimResult<ChainSpecT::RpcTransaction>>, ProviderErrorForChainSpec<ChainSpecT>> {
    // quick check even though we will also check in standardize_blocks with empty
    // blocks in between
    if simulate_payload.block_state_calls.len() > MAX_SIMULATE_BLOCKS {
        return Err(ProviderError::InvalidInput(format!(
            "Too many block state calls: {}. Maximum allowed is {}",
            simulate_payload.block_state_calls.len(),
            MAX_SIMULATE_BLOCKS
        )));
    }

    if simulate_payload.block_state_calls.is_empty() {
        return Err(ProviderError::InvalidInput(
            "No block state calls provided".to_string(),
        ));
    }

    let block_spec = block_spec.unwrap_or_else(BlockSpec::latest);

    let SimulatePayload {
        block_state_calls,
        trace_transfers, // TODO: use this
        validation,
        return_full_transactions, // TODO: use this
    } = simulate_payload;

    let mut parent_block = if let Some(block) = data.block_by_block_spec(&block_spec)? {
        block.header().clone()
    } else {
        return Err(ProviderError::InvalidInput(format!(
            "Block not found for block spec: {block_spec:?}"
        )));
    };
    let sim_blocks = standardize_blocks::<ChainSpecT, TimerT>(&parent_block, &block_state_calls)?;

    let mut prev_state = data.get_or_compute_state(parent_block.number)?;

    let mut cfg_env = data.create_evm_config_at_block_spec(&block_spec)?;
    cfg_env.disable_eip3607 = true;

    if !validation {
        cfg_env.disable_base_fee = true;
        cfg_env.disable_nonce_check = true;
    }

    let hardfork = data.hardfork_at_block_spec(&block_spec)?;

    let mut simulated_blocks = Vec::new();

    for block in sim_blocks {
        let SimBlock::<ChainSpecT::RpcTransactionRequest> {
            block_overrides,
            state_overrides,
            calls,
        } = block;

        let state_overrides =
            state_overrides.map_or(Ok(StateOverrides::default()), StateOverrides::try_from)?;

        let mut header_overrides = block_overrides
            .map(HeaderOverrides::<ChainSpecT::Hardfork>::from)
            .unwrap_or_default();

        if !validation {
            // disable base fee
            header_overrides.base_fee = Some(0);
            header_overrides.base_fee_params = None; // TODO: check if needed
        }

        // Fake sign transactions
        let calls = calls
            .into_iter()
            .map(|request| {
                let sender = *request.sender();
                let context = TransactionContext { data };
                let request = ChainSpecT::TransactionRequest::from_rpc_type(request, context)?;
                let transaction = request.fake_sign(sender);

                transaction::validate(transaction, hardfork.into())
                    .map_err(ProviderError::TransactionCreationError)
            })
            .collect::<Result<Vec<_>, _>>()?;

        let result = data.mine_block_with_multiple_transactions(
            &cfg_env,
            (*prev_state).clone(),
            header_overrides,
            &state_overrides,
            calls,
            &parent_block,
        )?;

        let MineBlockResultAndState {
            block,
            state,
            state_diff,
            transaction_results,
        } = result;

        // ! block has transaction receipts function

        // Prepare for the next iteration
        parent_block = block.header().clone();
        prev_state = state.into();

        // TODO: trace_transfers, return_full_transactions
        // TODO: do I need to clone the blockchain?

        let total_difficutly = None; // TODO: compute total difficulty

        let block = ChainSpecT::cast_local_block(block.into());
        let block = block_to_rpc_output::<ChainSpecT, TimerT>(
            hardfork,
            block,
            false,
            total_difficutly,
            return_full_transactions,
        )?;

        let sim_block_reult = SimResult::<ChainSpecT::RpcTransaction> {
            block,
            calls: transaction_results
                .into_iter()
                .map(|tx_result| {
                    let (status, return_data, gas_used, logs, error) = match tx_result {
                        ExecutionResult::Success {
                            reason,
                            gas_used,
                            gas_refunded,
                            logs,
                            output,
                        } => (true, output.into_data(), gas_used, logs, None),
                        ExecutionResult::Revert { gas_used, output } => {
                            let error = SimError {
                                code: -32000,
                                message: format!("Execution reverted"),
                            };
                            (false, output, gas_used, vec![], Some(error))
                        }
                        ExecutionResult::Halt { reason, gas_used } => {
                            // TODO: Return data for Halt?
                            let error = SimError {
                                code: -32015,
                                message: format!("VM execution error"),
                            };
                            (
                                false,
                                edr_eth::Bytes::from(vec![]),
                                gas_used,
                                vec![],
                                Some(error),
                            )
                        }
                    };

                    // TODO: Convert logs to LogOutput?
                    SimCallResult {
                        status,
                        return_data,
                        gas_used,
                        logs: vec![],
                        error,
                    }
                })
                .collect(),
        };

        simulated_blocks.push(sim_block_reult);
    }

    Ok(simulated_blocks)
}

/*
Rules:
    1. Blocks must be striclty increasing in number (and timestamp!?)
    2. If block overrides is None, create a default one
    3. Fill fields that are missing in block overrides with defaults

 */

fn standardize_blocks<
    ChainSpecT: SyncProviderSpec<
        TimerT,
        BlockEnv: Default,
        SignedTransaction: Clone
                               + Default
                               + TransactionValidation<
            ValidationError: From<EvmTransactionValidationError> + PartialEq,
        >,
    >,
    TimerT: Clone + TimeSinceEpoch,
>(
    base_block: &block::Header,
    block_state_calls: &Vec<SimBlock<ChainSpecT::RpcTransactionRequest>>,
) -> Result<Vec<SimBlock<ChainSpecT::RpcTransactionRequest>>, ProviderErrorForChainSpec<ChainSpecT>>
{
    let mut res_sim_blocks = Vec::new();

    let mut prev_block_number = base_block.number;
    let mut prev_block_timestamp = base_block.timestamp;

    for block_state_call in block_state_calls {
        let mut block = block_state_call.clone();

        if block.block_overrides.is_none() {
            block.block_overrides = Some(BlockOverrides::default());
        }

        // validate and fill in block overrides
        if let Some(block_overrides) = &mut block.block_overrides {
            let block_number = block_overrides.number.unwrap_or(prev_block_number + 1);
            if block_number <= prev_block_number {
                return Err(ProviderError::InvalidInput(format!(
                    "Block numbers must be strictly increasing. Previous: {}, current: {}",
                    prev_block_number, block_number
                )));
            }
            if block_number - base_block.number > MAX_SIMULATE_BLOCKS as u64 {
                return Err(ProviderError::InvalidInput(format!(
                    "Too many blocks. Maximum allowed is {}",
                    MAX_SIMULATE_BLOCKS as u64
                )));
            }
            block_overrides.number = Some(block_number);

            if block_number - prev_block_number > 1 {
                // fill empty blocks in between
                for empty_block_number in (prev_block_number + 1)..block_number {
                    let empty_block = SimBlock {
                        block_overrides: Some(BlockOverrides {
                            number: Some(empty_block_number),
                            time: Some(prev_block_timestamp + TIMESTAMP_INCREMENT),
                            ..Default::default()
                        }),
                        state_overrides: None,
                        calls: vec![],
                    };
                    prev_block_timestamp += TIMESTAMP_INCREMENT;
                    res_sim_blocks.push(empty_block);
                }
            }

            let block_time = block_overrides
                .time
                .unwrap_or(prev_block_timestamp + TIMESTAMP_INCREMENT);
            if block_time <= prev_block_timestamp {
                return Err(ProviderError::InvalidInput(format!(
                    "Block timestamps must be strictly increasing. Previous: {}, current: {}",
                    prev_block_timestamp, block_time
                )));
            }
            block_overrides.time = Some(block_time);

            prev_block_number = block_number;
            prev_block_timestamp = block_time;

            res_sim_blocks.push(block);
        }
    }
    Ok(res_sim_blocks)
}
