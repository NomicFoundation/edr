use core::fmt::Debug;
use std::iter;

use edr_eth::{
    filter::{FilteredEvents, LogFilterOptions, LogOutput, OneOrMore, SubscriptionType},
    BlockSpec, BlockTag, Eip1898BlockSpec, U256,
};
use edr_evm::HashSet;

use crate::{
    data::ProviderData, filter::LogFilter, requests::validation::validate_post_merge_block_tags,
    time::TimeSinceEpoch, ProviderError,
};

pub fn handle_get_filter_changes_request<LoggerErrorT: Debug, TimerT: Clone + TimeSinceEpoch>(
    data: &mut ProviderData<LoggerErrorT, TimerT>,
    filter_id: U256,
) -> Result<Option<FilteredEvents>, ProviderError<LoggerErrorT>> {
    Ok(data.get_filter_changes(&filter_id))
}

pub fn handle_get_filter_logs_request<LoggerErrorT: Debug, TimerT: Clone + TimeSinceEpoch>(
    data: &mut ProviderData<LoggerErrorT, TimerT>,
    filter_id: U256,
) -> Result<Option<Vec<LogOutput>>, ProviderError<LoggerErrorT>> {
    data.get_filter_logs(&filter_id)
}

pub fn handle_get_logs_request<LoggerErrorT: Debug, TimerT: Clone + TimeSinceEpoch>(
    data: &ProviderData<LoggerErrorT, TimerT>,
    filter_options: LogFilterOptions,
) -> Result<Vec<LogOutput>, ProviderError<LoggerErrorT>> {
    // Hardhat integration tests expect validation in this order.
    if let Some(from_block) = &filter_options.from_block {
        validate_post_merge_block_tags(data.spec_id(), from_block)?;
    }
    if let Some(to_block) = &filter_options.to_block {
        validate_post_merge_block_tags(data.spec_id(), to_block)?;
    }

    let filter = validate_filter_criteria::<true, LoggerErrorT, TimerT>(data, filter_options)?;
    data.logs(filter)
        .map(|logs| logs.iter().map(LogOutput::from).collect())
}

pub fn handle_new_block_filter_request<LoggerErrorT: Debug, TimerT: Clone + TimeSinceEpoch>(
    data: &mut ProviderData<LoggerErrorT, TimerT>,
) -> Result<U256, ProviderError<LoggerErrorT>> {
    data.add_block_filter::<false>()
}

pub fn handle_new_log_filter_request<LoggerErrorT: Debug, TimerT: Clone + TimeSinceEpoch>(
    data: &mut ProviderData<LoggerErrorT, TimerT>,
    filter_criteria: LogFilterOptions,
) -> Result<U256, ProviderError<LoggerErrorT>> {
    let filter_criteria =
        validate_filter_criteria::<false, LoggerErrorT, TimerT>(data, filter_criteria)?;
    data.add_log_filter::<false>(filter_criteria)
}

pub fn handle_new_pending_transaction_filter_request<
    LoggerErrorT: Debug,
    TimerT: Clone + TimeSinceEpoch,
>(
    data: &mut ProviderData<LoggerErrorT, TimerT>,
) -> Result<U256, ProviderError<LoggerErrorT>> {
    Ok(data.add_pending_transaction_filter::<false>())
}

pub fn handle_subscribe_request<LoggerErrorT: Debug, TimerT: Clone + TimeSinceEpoch>(
    data: &mut ProviderData<LoggerErrorT, TimerT>,
    subscription_type: SubscriptionType,
    filter_criteria: Option<LogFilterOptions>,
) -> Result<U256, ProviderError<LoggerErrorT>> {
    match subscription_type {
        SubscriptionType::Logs => {
            let filter_criteria = filter_criteria.ok_or_else(|| {
                ProviderError::InvalidArgument("Missing params argument".to_string())
            })?;
            let filter_criteria =
                validate_filter_criteria::<false, LoggerErrorT, TimerT>(data, filter_criteria)?;
            data.add_log_filter::<true>(filter_criteria)
        }
        SubscriptionType::NewHeads => data.add_block_filter::<true>(),
        SubscriptionType::NewPendingTransactions => {
            Ok(data.add_pending_transaction_filter::<true>())
        }
    }
}

pub fn handle_uninstall_filter_request<LoggerErrorT: Debug, TimerT: Clone + TimeSinceEpoch>(
    data: &mut ProviderData<LoggerErrorT, TimerT>,
    filter_id: U256,
) -> Result<bool, ProviderError<LoggerErrorT>> {
    Ok(data.remove_filter(&filter_id))
}

pub fn handle_unsubscribe_request<LoggerErrorT: Debug, TimerT: Clone + TimeSinceEpoch>(
    data: &mut ProviderData<LoggerErrorT, TimerT>,
    filter_id: U256,
) -> Result<bool, ProviderError<LoggerErrorT>> {
    Ok(data.remove_subscription(&filter_id))
}

fn validate_filter_criteria<
    const SHOULD_RESOLVE_LATEST: bool,
    LoggerErrorT: Debug,
    TimerT: Clone + TimeSinceEpoch,
>(
    data: &ProviderData<LoggerErrorT, TimerT>,
    filter: LogFilterOptions,
) -> Result<LogFilter, ProviderError<LoggerErrorT>> {
    fn normalize_block_spec<LoggerErrorT: Debug, TimerT: Clone + TimeSinceEpoch>(
        data: &ProviderData<LoggerErrorT, TimerT>,
        block_spec: Option<BlockSpec>,
    ) -> Result<Option<u64>, ProviderError<LoggerErrorT>> {
        if let Some(block_spec) = &block_spec {
            validate_post_merge_block_tags(data.spec_id(), block_spec)?;
        }

        let block_number = match block_spec {
            Some(
                BlockSpec::Number(block_number)
                | BlockSpec::Eip1898(Eip1898BlockSpec::Number { block_number }),
            ) => Some(block_number),
            Some(BlockSpec::Tag(BlockTag::Earliest)) => Some(0),
            Some(BlockSpec::Tag(
                BlockTag::Latest | BlockTag::Pending | BlockTag::Safe | BlockTag::Finalized,
            ))
            | None => None,
            Some(BlockSpec::Eip1898(Eip1898BlockSpec::Hash { block_hash, .. })) => {
                let block =
                    data.block_by_hash(&block_hash)?
                        .ok_or(ProviderError::InvalidArgument(
                            "blockHash cannot be found".to_string(),
                        ))?;
                Some(block.header().number)
            }
        };

        Ok(block_number)
    }

    let (from_block, to_block) = if let Some(block_hash) = filter.block_hash {
        if filter.from_block.is_some() || filter.to_block.is_some() {
            return Err(ProviderError::InvalidArgument(
                "blockHash is mutually exclusive with fromBlock/toBlock".to_string(),
            ));
        }

        let block = data.block_by_hash(&block_hash)?.ok_or_else(|| {
            ProviderError::InvalidArgument("blockHash cannot be found".to_string())
        })?;

        let block_number = block.header().number;

        (block_number, Some(block_number))
    } else {
        let from_block = normalize_block_spec(data, filter.from_block)?
            .unwrap_or_else(|| data.last_block_number());

        let mut to_block = normalize_block_spec(data, filter.to_block)?;
        if SHOULD_RESOLVE_LATEST && to_block.is_none() {
            to_block = Some(data.last_block_number());
        }

        (from_block, to_block)
    };

    let addresses = filter
        .address
        .map_or(HashSet::new(), |addresses| match addresses {
            OneOrMore::One(address) => iter::once(address).collect(),
            OneOrMore::Many(addresses) => addresses.into_iter().collect(),
        });

    let normalized_topics = filter.topics.map_or(Vec::new(), |topics| {
        topics
            .into_iter()
            .map(|topics| {
                topics.map(|topics| match topics {
                    OneOrMore::One(topic) => vec![topic],
                    OneOrMore::Many(topics) => topics,
                })
            })
            .collect()
    });

    Ok(LogFilter {
        from_block,
        to_block,
        addresses,
        normalized_topics,
    })
}
