use edr_eth::{
    block::{self, overrides, HeaderOverrides},
    l1::{self, BlockEnv},
    transaction::{request::TransactionRequestAndSender, TransactionValidation},
    BlockSpec,
};
use edr_evm::{
    state::{DatabaseComponents, State, StateOverrides, StateRefOverrider, WrapDatabaseRef},
    Block, BlockBuilder,
};
use edr_rpc_eth::{
    simulate::{SimBlock, SimResult, SimulatePayload},
    BlockOverrides,
};

use crate::{
    data::ProviderData,
    error::ProviderErrorForChainSpec,
    spec::{FromRpcType, Sender, SyncProviderSpec, TransactionContext},
    time::TimeSinceEpoch,
    ProviderError,
};

const MAX_SIMULATE_BLOCKS: usize = 256;
// TODO: do we check this?
const MAX_WITHDRAWALS: usize = 16;
// TODO: does this depend on the chain?
const TIMESTAMP_INCREMENT: u64 = 12;

// TODO: move some functionality to data.rs to avoid making functions public
pub fn handle_simulatev1_request<
    ChainSpecT: SyncProviderSpec<
        TimerT,
        BlockEnv: Default,
        SignedTransaction: Clone
                               + Default
                               + TransactionValidation<
            ValidationError: From<l1::InvalidTransaction> + PartialEq,
        >,
    >,
    TimerT: Clone + TimeSinceEpoch,
>(
    data: &mut ProviderData<ChainSpecT, TimerT>,
    simulate_payload: SimulatePayload<ChainSpecT::RpcTransactionRequest>,
    block_spec: Option<BlockSpec>,
) -> Result<SimResult, ProviderErrorForChainSpec<ChainSpecT>> {
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

    // data + block_spec is our base context
    // CfgEnv for EVM config - create_evm_config, create_evm_config_at_block_spec

    let block_spec = block_spec.unwrap_or_else(BlockSpec::latest);

    let SimulatePayload {
        block_state_calls,
        trace_transfers,
        validation,
        return_full_transactions,
    } = simulate_payload;

    let mut parent_block = if let Some(block) = data.block_by_block_spec(&block_spec)? {
        block.header().clone()
    } else {
        return Err(ProviderError::InvalidInput(format!(
            "Block not found for block spec: {:?}",
            block_spec
        )));
    };
    let sim_blocks = standardize_blocks::<ChainSpecT, TimerT>(&parent_block, &block_state_calls)?;

    let state = data.get_or_compute_state(parent_block.number)?;

    let mut cfg_env = data.create_evm_config_at_block_spec(&block_spec)?;

    for block in sim_blocks {
        let mut block_env =
            ChainSpecT::new_block_env_from_parent(&parent_block, cfg_env.spec.into());

        // configure EVM env
        // may not be enough to start building all the blocks from here? or is state
        // enough?
        // let mut cfg_env = data.create_evm_config_at_block_spec(&last_block_spec)?;

        // update cfg_env
        cfg_env.disable_eip3607 = true;

        if !validation {
            cfg_env.disable_base_fee = true;
            cfg_env.disable_nonce_check = true;
            // block env base fee?
        }

        let SimBlock::<ChainSpecT::RpcTransactionRequest> {
            block_overrides,
            state_overrides,
            calls,
        } = block;

        let state_overrides =
            state_overrides.map_or(Ok(StateOverrides::default()), StateOverrides::try_from)?;

        // check gas limits
        //     if let Some(block_overrides) = block_overrides {
        //         if let Some(gas_limit_override) = block_overrides.gas_limit {
        //             if gas_limit_override >
        // ChainSpecT::BlockEnv::MAX_GAS_LIMIT {                 return
        // Err(ProviderError::InvalidInput(format!(
        // "Gas limit override too high: {}. Maximum allowed is {}",
        //                     gas_limit_override,
        //                     ChainSpecT::BlockEnv::MAX_GAS_LIMIT
        //                 )));
        //             }
        //         }
        //     }
        // }

        // let block_env = ChainSpecT::new_block_env(header, cfg_env.spec.into());

        apply_block_overrides(block_overrides, block_env);

        apply_state_overrides(state, state_overrides);

        for call in calls {
            let sender = call.sender();

            let context = TransactionContext { data };
            let request = ChainSpecT::TransactionRequest::from_rpc_type(call, context)?;

            let request = TransactionRequestAndSender {
                request,
                sender: *sender,
            };
            let signed_transaction = data.sign_transaction_request(request)?;

            // add transaction to mempool
            data.add_pending_transaction(signed_transaction)?;
        }

        let result =
            data.mine_and_commit_block(HeaderOverrides::from(block_overrides.unwrap_or_default()))?;
    }

    Ok(res)
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
            ValidationError: From<l1::InvalidTransaction> + PartialEq,
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

fn apply_block_overrides(overrides: &BlockOverrides, block_env: &mut BlockEnv) {
    if let Some(number) = overrides.number {
        block_env.number = U256::from(number);
    }
    if let Some(time) = overrides.time {
        block_env.timestamp = U256::from(time);
    }
    if let Some(gas_limit) = overrides.gas_limit {
        block_env.gas_limit = gas_limit;
    }
    if let Some(fee_recipient) = overrides.fee_recipient {
        block_env.beneficiary = fee_recipient;
    }
    if let Some(base_fee_per_gas) = overrides.base_fee_per_gas {
        block_env.basefee = base_fee_per_gas.as_u64();
    }
}

fn apply_state_overrides<
    ChainSpecT: SyncProviderSpec<
        TimerT,
        BlockEnv: Default,
        SignedTransaction: Clone
                               + Default
                               + TransactionValidation<
            ValidationError: From<l1::InvalidTransaction> + PartialEq,
        >,
    >,
    TimerT: Clone + TimeSinceEpoch,
    StateT: State<Error: Send + std::error::Error> + AsRef<StateT>,
>(
    state: StateT,
    state_overrides: &StateOverrides,
) -> Result<StateT, ProviderErrorForChainSpec<ChainSpecT>> {
    let state_overrider = StateRefOverrider::new(&state_overrides, state.as_ref());

    Ok(state)
}
