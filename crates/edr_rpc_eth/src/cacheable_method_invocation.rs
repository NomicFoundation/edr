use edr_eth::{
    block::{is_safe_block_number, IsSafeBlockNumberArgs},
    reward_percentile::RewardPercentile,
    Address, B256, U256,
};
use edr_rpc_client::{CacheKeyHasher, CacheableMethod};
use sha3::{digest::FixedOutput, Digest, Sha3_256};

use crate::{
    filter::{LogFilterOptions, OneOrMore},
    request_methods::RequestMethod,
    BlockSpec, BlockTag, Eip1898BlockSpec, PreEip1898BlockSpec,
};

pub(super) fn try_read_cache_key(method: &RequestMethod) -> Option<ReadCacheKey> {
    CacheableRequestMethod::try_from(method)
        .ok()
        .and_then(CacheableRequestMethod::read_cache_key)
}

pub(super) fn try_write_cache_key(method: &RequestMethod) -> Option<WriteCacheKey> {
    CacheableRequestMethod::try_from(method)
        .ok()
        .and_then(CacheableRequestMethod::write_cache_key)
}

/// Potentially cacheable Ethereum JSON-RPC methods.
#[derive(Clone, Debug)]
enum CacheableRequestMethod<'a> {
    /// `eth_feeHistory`
    FeeHistory {
        block_count: &'a U256,
        newest_block: CacheableBlockSpec<'a>,
        reward_percentiles: &'a Option<Vec<RewardPercentile>>,
    },
    /// `eth_getBalance`
    GetBalance {
        address: &'a Address,
        block_spec: CacheableBlockSpec<'a>,
    },
    /// `eth_getBlockByNumber`
    GetBlockByNumber {
        block_spec: CacheableBlockSpec<'a>,

        /// include transaction data
        include_tx_data: bool,
    },
    /// `eth_getBlockByHash`
    GetBlockByHash {
        /// hash
        block_hash: &'a B256,
        /// include transaction data
        include_tx_data: bool,
    },
    /// `eth_getCode`
    GetCode {
        address: &'a Address,
        block_spec: CacheableBlockSpec<'a>,
    },
    /// `eth_getLogs`
    GetLogs {
        params: CacheableLogFilterOptions<'a>,
    },
    /// `eth_getStorageAt`
    GetStorageAt {
        address: &'a Address,
        position: &'a U256,
        block_spec: CacheableBlockSpec<'a>,
    },
    /// `eth_getTransactionByHash`
    GetTransactionByHash { transaction_hash: &'a B256 },
    /// `eth_getTransactionCount`
    GetTransactionCount {
        address: &'a Address,
        block_spec: CacheableBlockSpec<'a>,
    },
    /// `eth_getTransactionReceipt`
    GetTransactionReceipt { transaction_hash: &'a B256 },
    /// `net_version`
    NetVersion,
}

impl<'a> CacheableRequestMethod<'a> {
    // Allow to keep same structure as other RequestMethod and other methods.
    #[allow(clippy::match_same_arms)]
    fn key_hasher(self) -> Result<CacheKeyHasher, SymbolicBlogTagError> {
        let hasher = CacheKeyHasher::new();
        let hasher = hasher.hash_u8(method.cache_key_variant());

        let hasher = match method {
            CacheableRequestMethod::FeeHistory {
                block_count,
                newest_block,
                reward_percentiles,
            } => {
                let hasher = hasher
                    .hash_u256(block_count)
                    .hash_block_spec(newest_block)?
                    .hash_u8(reward_percentiles.cache_key_variant());
                match reward_percentiles {
                    Some(reward_percentiles) => hasher.hash_reward_percentiles(reward_percentiles),
                    None => hasher,
                }
            }
            CacheableRequestMethod::GetBalance {
                address,
                block_spec,
            } => hasher.hash_address(address).hash_block_spec(block_spec)?,
            CacheableRequestMethod::GetBlockByNumber {
                block_spec,
                include_tx_data,
            } => hasher
                .hash_block_spec(block_spec)?
                .hash_bool(include_tx_data),
            CacheableRequestMethod::GetBlockByHash {
                block_hash,
                include_tx_data,
            } => hasher.hash_b256(block_hash).hash_bool(include_tx_data),
            CacheableRequestMethod::GetCode {
                address,
                block_spec,
            } => hasher.hash_address(address).hash_block_spec(block_spec)?,
            CacheableRequestMethod::GetLogs { params } => hasher.hash_log_filter_options(params)?,
            CacheableRequestMethod::GetStorageAt {
                address,
                position,
                block_spec,
            } => hasher
                .hash_address(address)
                .hash_u256(position)
                .hash_block_spec(block_spec)?,
            CacheableRequestMethod::GetTransactionByHash { transaction_hash } => {
                hasher.hash_b256(transaction_hash)
            }
            CacheableRequestMethod::GetTransactionCount {
                address,
                block_spec,
            } => hasher.hash_address(address).hash_block_spec(block_spec)?,
            CacheableRequestMethod::GetTransactionReceipt { transaction_hash } => {
                hasher.hash_b256(transaction_hash)
            }
            CacheableRequestMethod::NetVersion => hasher,
        };

        Ok(hasher)
    }
}

/// Error type for [`CacheableRequestMethod::try_from`].
#[derive(thiserror::Error, Debug)]
enum MethodNotCacheableError {
    #[error(transparent)]
    BlockSpec(#[from] BlockSpecNotCacheableError),
    #[error("Method is not cacheable: {0:?}")]
    RequestMethod(RequestMethod),
    #[error("Get logs input is not cacheable: {0:?}")]
    GetLogsInput(#[from] LogFilterOptionsNotCacheableError),
    #[error(transparent)]
    PreEip18989BlockSpec(#[from] PreEip1898BlockSpecNotCacheableError),
}

impl<'a> TryFrom<&'a RequestMethod> for CacheableRequestMethod<'a> {
    type Error = MethodNotCacheableError;

    fn try_from(value: &'a RequestMethod) -> Result<Self, Self::Error> {
        match value {
            RequestMethod::FeeHistory(block_count, newest_block, reward_percentiles) => {
                Ok(CacheableRequestMethod::FeeHistory {
                    block_count,
                    newest_block: newest_block.try_into()?,
                    reward_percentiles,
                })
            }
            RequestMethod::GetBalance(address, block_spec) => {
                Ok(CacheableRequestMethod::GetBalance {
                    address,
                    block_spec: block_spec.try_into()?,
                })
            }
            RequestMethod::GetBlockByNumber(block_spec, include_tx_data) => {
                Ok(CacheableRequestMethod::GetBlockByNumber {
                    block_spec: block_spec.try_into()?,
                    include_tx_data: *include_tx_data,
                })
            }
            RequestMethod::GetBlockByHash(block_hash, include_tx_data) => {
                Ok(CacheableRequestMethod::GetBlockByHash {
                    block_hash,
                    include_tx_data: *include_tx_data,
                })
            }
            RequestMethod::GetCode(address, block_spec) => Ok(CacheableRequestMethod::GetCode {
                address,
                block_spec: block_spec.try_into()?,
            }),
            RequestMethod::GetLogs(params) => Ok(CacheableRequestMethod::GetLogs {
                params: params.try_into()?,
            }),
            RequestMethod::GetStorageAt(address, position, block_spec) => {
                Ok(CacheableRequestMethod::GetStorageAt {
                    address,
                    position,
                    block_spec: block_spec.try_into()?,
                })
            }
            RequestMethod::GetTransactionByHash(transaction_hash) => {
                Ok(CacheableRequestMethod::GetTransactionByHash { transaction_hash })
            }
            RequestMethod::GetTransactionCount(address, block_spec) => {
                Ok(CacheableRequestMethod::GetTransactionCount {
                    address,
                    block_spec: block_spec.try_into()?,
                })
            }
            RequestMethod::GetTransactionReceipt(transaction_hash) => {
                Ok(CacheableRequestMethod::GetTransactionReceipt { transaction_hash })
            }
            RequestMethod::NetVersion(_) => Ok(CacheableRequestMethod::NetVersion),

            // Explicit to make sure if a new method is added, it is not forgotten here.
            // Chain id is not cacheable since a remote might change its chain id e.g. if it's a
            // forked node running on localhost.
            RequestMethod::BlockNumber(_) | RequestMethod::ChainId(_) => {
                Err(MethodNotCacheableError::RequestMethod(value.clone()))
            }
        }
    }
}

/// Method invocations where, if the block spec argument is symbolic, it can be
/// resolved to a block number from the response.
#[derive(Debug, Clone)]
enum MethodWithResolvableSymbolicBlockSpec {
    GetBlockByNumber { include_tx_data: bool },
}

impl MethodWithResolvableSymbolicBlockSpec {
    fn new(method: CacheableRequestMethod<'_>) -> Option<Self> {
        match method {
            CacheableRequestMethod::GetBlockByNumber {
                include_tx_data,
                block_spec: _,
            } => Some(Self::GetBlockByNumber { include_tx_data }),
            _ => None,
        }
    }
}

impl<'a> CacheableMethod for RequestMethod<'a> {
    type MethodWithResolvableBlockTag = MethodWithResolvableSymbolicBlockSpec;

    fn block_number_request() -> Self {
        Self::BlockNumber(())
    }

    fn chain_id_request() -> Self {
        Self::ChainId(())
    }

    fn resolve_block_tag(method: Self::MethodWithResolvableBlockTag, block_number: u64) -> Self {
        match self.method {
            MethodWithResolvableSymbolicBlockSpec::GetBlockByNumber {
                include_tx_data, ..
            } => CacheableRequestMethod::GetBlockByNumber {
                block_spec: resolved_block_spec,
                include_tx_data,
            },
        };
    }

    fn read_cache_key(self) -> Option<ReadCacheKey> {
        let cacheable_method = CacheableRequestMethod::try_from(&self).ok()?;

        let cache_key = cacheable_method.key_hasher().ok()?.finalize();
        Some(ReadCacheKey(cache_key))
    }

    #[allow(clippy::match_same_arms)]
    fn write_cache_key(self) -> Option<WriteCacheKey> {
        let cacheable_method = CacheableRequestMethod::try_from(&self).ok()?;

        match cacheable_method.key_hasher() {
            Err(SymbolicBlogTagError) => WriteCacheKey::needs_block_number(self),
            Ok(hasher) => match self {
                CacheableRequestMethod::FeeHistory {
                    block_count: _,
                    newest_block,
                    reward_percentiles: _,
                } => WriteCacheKey::needs_safety_check(hasher, newest_block),
                CacheableRequestMethod::GetBalance {
                    address: _,
                    block_spec,
                } => WriteCacheKey::needs_safety_check(hasher, block_spec),
                CacheableRequestMethod::GetBlockByNumber {
                    block_spec,
                    include_tx_data: _,
                } => WriteCacheKey::needs_safety_check(hasher, block_spec),
                CacheableRequestMethod::GetBlockByHash {
                    block_hash: _,
                    include_tx_data: _,
                } => Some(WriteCacheKey::finalize(hasher)),
                CacheableRequestMethod::GetCode {
                    address: _,
                    block_spec,
                } => WriteCacheKey::needs_safety_check(hasher, block_spec),
                CacheableRequestMethod::GetLogs {
                    params: CacheableLogFilterOptions { range, .. },
                } => WriteCacheKey::needs_range_check(hasher, range),
                CacheableRequestMethod::GetStorageAt {
                    address: _,
                    position: _,
                    block_spec,
                } => WriteCacheKey::needs_safety_check(hasher, block_spec),
                CacheableRequestMethod::GetTransactionByHash {
                    transaction_hash: _,
                } => Some(WriteCacheKey::finalize(hasher)),
                CacheableRequestMethod::GetTransactionCount {
                    address: _,
                    block_spec,
                } => WriteCacheKey::needs_safety_check(hasher, block_spec),
                CacheableRequestMethod::GetTransactionReceipt {
                    transaction_hash: _,
                } => Some(WriteCacheKey::finalize(hasher)),
                CacheableRequestMethod::NetVersion => Some(WriteCacheKey::finalize(hasher)),
            },
        }
    }
}

/// Error type for [`CacheableBlockSpec::try_from`].
#[derive(thiserror::Error, Debug)]
#[error("Block spec is not cacheable: {0:?}")]
struct PreEip1898BlockSpecNotCacheableError(PreEip1898BlockSpec);

impl<'a> TryFrom<&'a PreEip1898BlockSpec> for CacheableBlockSpec<'a> {
    type Error = PreEip1898BlockSpecNotCacheableError;

    fn try_from(value: &'a PreEip1898BlockSpec) -> Result<Self, Self::Error> {
        match value {
            PreEip1898BlockSpec::Number(block_number) => Ok(CacheableBlockSpec::Number {
                block_number: *block_number,
            }),
            PreEip1898BlockSpec::Tag(tag) => match tag {
                // Latest and pending can never be resolved to a safe block number.
                BlockTag::Latest | BlockTag::Pending => {
                    Err(PreEip1898BlockSpecNotCacheableError(value.clone()))
                }
                // Earliest, safe and finalized are potentially resolvable to a safe block number.
                BlockTag::Earliest => Ok(CacheableBlockSpec::Earliest),
                BlockTag::Safe => Ok(CacheableBlockSpec::Safe),
                BlockTag::Finalized => Ok(CacheableBlockSpec::Finalized),
            },
        }
    }
}

impl<'a> CacheKeyVariant for &'a CacheableRequestMethod<'a> {
    fn cache_key_variant(&self) -> u8 {
        match self {
            // The commented out methods have been removed as they're not currently in use by the
            // RPC client. If they're added back, they should keep their old variant
            // number. CacheableRequestMethod::ChainId => 0,
            CacheableRequestMethod::GetBalance { .. } => 1,
            CacheableRequestMethod::GetBlockByNumber { .. } => 2,
            CacheableRequestMethod::GetBlockByHash { .. } => 3,
            // CacheableRequestMethod::GetBlockTransactionCountByHash { .. } => 4,
            // CacheableRequestMethod::GetBlockTransactionCountByNumber { .. } => 5,
            CacheableRequestMethod::GetCode { .. } => 6,
            CacheableRequestMethod::GetLogs { .. } => 7,
            CacheableRequestMethod::GetStorageAt { .. } => 8,
            // CacheableRequestMethod::GetTransactionByBlockHashAndIndex { .. } => 9,
            // CacheableRequestMethod::GetTransactionByBlockNumberAndIndex { .. } => 10,
            CacheableRequestMethod::GetTransactionByHash { .. } => 11,
            CacheableRequestMethod::GetTransactionCount { .. } => 12,
            CacheableRequestMethod::GetTransactionReceipt { .. } => 13,
            CacheableRequestMethod::NetVersion => 14,
            CacheableRequestMethod::FeeHistory { .. } => 15,
        }
    }
}

// // Allow to keep same structure as other RequestMethod and other methods.
// #[allow(clippy::match_same_arms)]
// fn hash_method(
//     self,
//     method: &CacheableRequestMethod<'_>,
// ) -> Result<Self, SymbolicBlogTagError> {
//     let this = self.hash_u8(method.cache_key_variant());

//     let this = match method {
//         CacheableRequestMethod::FeeHistory {
//             block_count,
//             newest_block,
//             reward_percentiles,
//         } => {
//             let this = this
//                 .hash_u256(block_count)
//                 .hash_block_spec(newest_block)?
//                 .hash_u8(reward_percentiles.cache_key_variant());
//             match reward_percentiles {
//                 Some(reward_percentiles) =>
// this.hash_reward_percentiles(reward_percentiles),                 None =>
// this,             }
//         }
//         CacheableRequestMethod::GetBalance {
//             address,
//             block_spec,
//         } => this.hash_address(address).hash_block_spec(block_spec)?,
//         CacheableRequestMethod::GetBlockByNumber {
//             block_spec,
//             include_tx_data,
//         } => this.hash_block_spec(block_spec)?.hash_bool(include_tx_data),
//         CacheableRequestMethod::GetBlockByHash {
//             block_hash,
//             include_tx_data,
//         } => this.hash_b256(block_hash).hash_bool(include_tx_data),
//         CacheableRequestMethod::GetCode {
//             address,
//             block_spec,
//         } => this.hash_address(address).hash_block_spec(block_spec)?,
//         CacheableRequestMethod::GetLogs { params } =>
// this.hash_log_filter_options(params)?,
//         CacheableRequestMethod::GetStorageAt {
//             address,
//             position,
//             block_spec,
//         } => this
//             .hash_address(address)
//             .hash_u256(position)
//             .hash_block_spec(block_spec)?,
//         CacheableRequestMethod::GetTransactionByHash { transaction_hash } =>
// {             this.hash_b256(transaction_hash)
//         }
//         CacheableRequestMethod::GetTransactionCount {
//             address,
//             block_spec,
//         } => this.hash_address(address).hash_block_spec(block_spec)?,
//         CacheableRequestMethod::GetTransactionReceipt { transaction_hash } =>
// {             this.hash_b256(transaction_hash)
//         }
//         CacheableRequestMethod::NetVersion => this,
//     };

//     Ok(this)
// }

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_hash_length() {
        let hash = Hasher::new().hash_u8(0).finalize();
        // 32 bytes as hex
        assert_eq!(hash.len(), 2 * 32);
    }

    #[test]
    fn test_hasher_block_spec_hash_and_number_not_equal() {
        let block_number = u64::default();
        let block_hash = B256::default();

        let hash_one = Hasher::new()
            .hash_block_spec(&CacheableBlockSpec::Hash {
                block_hash: &block_hash,
                require_canonical: None,
            })
            .unwrap()
            .finalize();
        let hash_two = Hasher::new()
            .hash_block_spec(&CacheableBlockSpec::Number { block_number })
            .unwrap()
            .finalize();

        assert_ne!(hash_one, hash_two);
    }

    #[test]
    fn test_get_logs_input_from_to_matters() {
        let from = CacheableBlockSpec::Number { block_number: 1 };
        let to = CacheableBlockSpec::Number { block_number: 2 };
        let address = Address::default();

        let hash_one = Hasher::new()
            .hash_log_filter_options(&CacheableLogFilterOptions {
                range: CacheableLogFilterRange::Range {
                    from_block: from.clone(),
                    to_block: to.clone(),
                },
                address: vec![&address],
                topics: Vec::new(),
            })
            .unwrap()
            .finalize();

        let hash_two = Hasher::new()
            .hash_log_filter_options(&CacheableLogFilterOptions {
                range: CacheableLogFilterRange::Range {
                    from_block: to,
                    to_block: from,
                },
                address: vec![&address],
                topics: Vec::new(),
            })
            .unwrap()
            .finalize();

        assert_ne!(hash_one, hash_two);
    }

    #[test]
    fn test_same_arguments_keys_not_equal() {
        let value = B256::default();
        let key_one = CacheableRequestMethod::GetTransactionByHash {
            transaction_hash: &value,
        }
        .read_cache_key()
        .unwrap();
        let key_two = CacheableRequestMethod::GetTransactionReceipt {
            transaction_hash: &value,
        }
        .read_cache_key()
        .unwrap();

        assert_ne!(key_one, key_two);
    }

    #[test]
    fn test_get_storage_at_block_spec_is_taken_into_account() {
        let address = Address::default();
        let position = U256::default();

        let key_one = CacheableRequestMethod::GetStorageAt {
            address: &address,
            position: &position,
            block_spec: CacheableBlockSpec::Hash {
                block_hash: &B256::default(),
                require_canonical: None,
            },
        }
        .read_cache_key()
        .unwrap();

        let key_two = CacheableRequestMethod::GetStorageAt {
            address: &address,
            position: &position,
            block_spec: CacheableBlockSpec::Number {
                block_number: u64::default(),
            },
        }
        .read_cache_key()
        .unwrap();

        assert_ne!(key_one, key_two);
    }

    #[test]
    fn test_get_storage_at_block_same_matches() {
        let address = Address::default();
        let position = U256::default();
        let block_number = u64::default();
        let block_spec = CacheableBlockSpec::Number { block_number };

        let key_one = CacheableRequestMethod::GetStorageAt {
            address: &address,
            position: &position,
            block_spec: block_spec.clone(),
        }
        .read_cache_key()
        .unwrap();

        let key_two = CacheableRequestMethod::GetStorageAt {
            address: &address,
            position: &position,
            block_spec,
        }
        .read_cache_key()
        .unwrap();

        assert_eq!(key_one, key_two);
    }
}
