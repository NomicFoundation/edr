use edr_eth::block::{IsSafeBlockNumberArgs, is_safe_block_number};

use super::{
    CacheableMethod, block_spec::CacheableBlockSpec, filter::CacheableLogFilterRange,
    hasher::KeyHasher,
};

/// Trait for retrieving the unique id of an enum variant.
// This could be replaced by the unstable
// [`core::intrinsics::discriminant_value`](https://dev-doc.rust-lang.org/beta/core/intrinsics/fn.discriminant_value.html)
// function once it becomes stable.
pub trait CacheKeyVariant {
    /// Returns the unique id of the enum variant.
    fn cache_key_variant(&self) -> u8;
}

impl<T> CacheKeyVariant for Option<T> {
    fn cache_key_variant(&self) -> u8 {
        match self {
            None => 0,
            Some(_) => 1,
        }
    }
}

/// A cache key that can be used to read from the cache.
/// It's based on not-fully resolved data, so it's not safe to write to this
/// cache key. Specifically, it's not checked whether the block number is safe
/// to cache (safe from reorgs). This is ok for reading from the cache, since
/// the result will be a cache miss if the block number is not safe to cache and
/// not having to resolve this data for reading offers performance advantages.
#[derive(Clone, Debug, PartialEq, Eq)]
#[repr(transparent)]
pub struct ReadCacheKey(String);

impl ReadCacheKey {
    /// Finalizes the provided [`KeyHasher`] and return the resolved cache key.
    pub fn finalize(hasher: KeyHasher) -> Self {
        Self(hasher.finalize())
    }
}

impl AsRef<str> for ReadCacheKey {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// A cache key that can be used to write to the cache.
#[derive(Clone, Debug)]
pub enum WriteCacheKey<MethodT: CacheableMethod> {
    /// It needs to be checked whether the block number is safe (reorg-free)
    /// before writing to the cache.
    NeedsSafetyCheck(CacheKeyForUncheckedBlockNumber),
    /// The method invocation contains a symbolic block spec (e.g. "finalized")
    /// that needs to be resolved to a block number before the result can be
    /// cached.
    NeedsBlockTagResolution(CacheKeyForUnresolvedBlockTag<MethodT>),
    /// The cache key is fully resolved and can be used to write to the cache.
    Resolved(String),
}

impl<MethodT: CacheableMethod> WriteCacheKey<MethodT> {
    /// Finalizes the provided [`KeyHasher`] and return the resolved cache
    /// key.
    pub fn finalize(hasher: KeyHasher) -> Self {
        Self::Resolved(hasher.finalize())
    }

    /// Checks whether the block number is safe to cache before returning a
    /// cache key.
    pub fn needs_range_check(
        hasher: KeyHasher,
        range: CacheableLogFilterRange<'_>,
    ) -> Option<Self> {
        match range {
            CacheableLogFilterRange::Hash(_) => Some(Self::finalize(hasher)),
            CacheableLogFilterRange::Range { to_block, .. } => {
                // TODO should we check that to < from?
                Self::needs_safety_check(hasher, to_block)
            }
        }
    }

    /// Checks whether the block number is safe to cache before returning a
    /// cache key.
    pub fn needs_safety_check(
        hasher: KeyHasher,
        block_spec: CacheableBlockSpec<'_>,
    ) -> Option<Self> {
        match block_spec {
            CacheableBlockSpec::Number { block_number } => {
                Some(Self::NeedsSafetyCheck(CacheKeyForUncheckedBlockNumber {
                    hasher: Box::new(hasher),
                    block_number,
                }))
            }
            CacheableBlockSpec::Hash { .. } => Some(Self::finalize(hasher)),
            CacheableBlockSpec::Earliest
            | CacheableBlockSpec::Safe
            | CacheableBlockSpec::Finalized => None,
        }
    }

    /// Checks whether a block tag needs to be resolved before returning a cache
    /// key.
    pub fn needs_block_tag_resolution(method: MethodT) -> Option<Self> {
        let method = method.into()?;

        Some(Self::NeedsBlockTagResolution(
            CacheKeyForUnresolvedBlockTag { method },
        ))
    }
}

/// A cache key for which the block number needs to be checked before writing to
/// the cache.
#[derive(Clone, Debug)]
pub struct CacheKeyForUncheckedBlockNumber {
    // Boxed to keep the size of the enum small.
    hasher: Box<KeyHasher>,
    pub(super) block_number: u64,
}

impl CacheKeyForUncheckedBlockNumber {
    /// Check whether the block number is safe to cache before returning a cache
    /// key.
    pub fn validate_block_number(self, chain_id: u64, latest_block_number: u64) -> Option<String> {
        let is_safe = is_safe_block_number(IsSafeBlockNumberArgs {
            chain_id,
            latest_block_number,
            block_number: self.block_number,
        });
        if is_safe {
            Some(self.hasher.finalize())
        } else {
            None
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) enum ResolvedSymbolicTag {
    /// It needs to be checked whether the block number is safe (reorg-free)
    /// before writing to the cache.
    NeedsSafetyCheck(CacheKeyForUncheckedBlockNumber),
    /// The cache key is fully resolved and can be used to write to the cache.
    Resolved(String),
}

/// A cache key for which the block tag needs to be resolved before writing to
/// the cache.
#[derive(Clone, Debug)]
pub struct CacheKeyForUnresolvedBlockTag<MethodT: CacheableMethod> {
    method: MethodT::MethodWithResolvableBlockTag,
}

impl<MethodT: CacheableMethod> CacheKeyForUnresolvedBlockTag<MethodT> {
    /// Check whether the block number is safe to cache before returning a cache
    /// key.
    pub(crate) fn resolve_block_tag(self, block_number: u64) -> Option<ResolvedSymbolicTag> {
        let resolved_method = MethodT::resolve_block_tag(self.method, block_number);

        resolved_method.write_cache_key().map(|key| match key {
            WriteCacheKey::NeedsSafetyCheck(cache_key) => {
                ResolvedSymbolicTag::NeedsSafetyCheck(cache_key)
            }
            WriteCacheKey::Resolved(cache_key) => ResolvedSymbolicTag::Resolved(cache_key),
            WriteCacheKey::NeedsBlockTagResolution(_) => {
                unreachable!("resolved block spec should not need block number")
            }
        })
    }
}
