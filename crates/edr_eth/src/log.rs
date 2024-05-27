mod block;
mod filter;
mod receipt;

pub use revm_primitives::Log;

pub use self::{
    block::{BlockLog, FullBlockLog},
    filter::FilterLog,
    receipt::ReceiptLog,
};
use crate::{Address, Bloom, BloomInput, HashSet, B256};

/// Adds the log to a bloom hash.
pub fn add_log_to_bloom(log: &Log, bloom: &mut Bloom) {
    bloom.accrue(BloomInput::Raw(log.address.as_slice()));

    log.topics()
        .iter()
        .for_each(|topic| bloom.accrue(BloomInput::Raw(topic.as_slice())));
}

/// Whether the log address matches the address filter.
pub fn matches_address_filter(log_address: &Address, address_filter: &HashSet<Address>) -> bool {
    address_filter.is_empty() || address_filter.contains(log_address)
}

/// Whether the log topics match the topics filter.
pub fn matches_topics_filter(log_topics: &[B256], topics_filter: &[Option<Vec<B256>>]) -> bool {
    if topics_filter.len() > log_topics.len() {
        return false;
    }

    topics_filter
        .iter()
        .zip(log_topics.iter())
        .all(|(normalized_topics, log_topic)| {
            normalized_topics
                .as_ref()
                .map_or(true, |normalized_topics| {
                    normalized_topics
                        .iter()
                        .any(|normalized_topic| *normalized_topic == *log_topic)
                })
        })
}
