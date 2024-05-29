use edr_eth::{reward_percentile::RewardPercentile, Address, B256, U256};
use sha3::{digest::FixedOutput, Digest, Sha3_256};

use super::{
    block_spec::{CacheableBlockSpec, UnresolvedBlockTagError},
    filter::{CacheableLogFilterOptions, CacheableLogFilterRange},
    key::CacheKeyVariant,
};

#[derive(Debug, Clone)]
pub struct KeyHasher {
    hasher: Sha3_256,
}

// The methods take `mut self` instead of `&mut self` to make sure no hash is
// constructed if one of the method arguments are invalid (in which case the
// method returns None and consumes self).
//
// Before variants of an enum are hashed, a variant marker is hashed before
// hashing the values of the variants to distinguish between them. E.g. the hash
// of `Enum::Foo(1u8)` should not equal the hash of `Enum::Bar(1u8)`, since
// these are not logically equivalent. This matches the behavior of the `Hash`
// derivation of the Rust standard library for enums.
//
// Instead of ignoring `None` values, the same pattern is followed for Options
// in order to let us distinguish between `[None, Some("a")]` and `[Some("a")]`.
// Note that if we use the cache key variant `0u8` for `None`, it's ok if `None`
// and `0u8`, hash to the same values since a type where `Option` and `u8` are
// valid values must be wrapped in an enum in Rust and the enum cache key
// variant prefix will distinguish between them. This wouldn't be the case with
// JSON though.
//
// When adding new types such as sequences or strings, [prefix
// collisions](https://doc.rust-lang.org/std/hash/trait.Hash.html#prefix-collisions) should be
// considered.
impl KeyHasher {
    pub fn new() -> Self {
        Self {
            hasher: Sha3_256::new(),
        }
    }

    pub fn hash_bytes(mut self, bytes: impl AsRef<[u8]>) -> Self {
        self.hasher.update(bytes);

        self
    }

    pub fn hash_u8(self, value: u8) -> Self {
        self.hash_bytes(value.to_le_bytes())
    }

    pub fn hash_bool(self, value: bool) -> Self {
        self.hash_u8(u8::from(value))
    }

    pub fn hash_address(self, address: &Address) -> Self {
        self.hash_bytes(address)
    }

    pub fn hash_u64(self, value: u64) -> Self {
        self.hash_bytes(value.to_le_bytes())
    }

    pub fn hash_u256(self, value: &U256) -> Self {
        self.hash_bytes(value.as_le_bytes())
    }

    pub fn hash_b256(self, value: &B256) -> Self {
        self.hash_bytes(value)
    }

    pub fn hash_block_spec(
        self,
        block_spec: &CacheableBlockSpec<'_>,
    ) -> Result<Self, UnresolvedBlockTagError> {
        let this = self.hash_u8(block_spec.cache_key_variant());

        match block_spec {
            CacheableBlockSpec::Number { block_number } => Ok(this.hash_u64(*block_number)),
            CacheableBlockSpec::Hash {
                block_hash,
                require_canonical,
            } => {
                let this = this
                    .hash_b256(block_hash)
                    .hash_u8(require_canonical.cache_key_variant());
                match require_canonical {
                    Some(require_canonical) => Ok(this.hash_bool(*require_canonical)),
                    None => Ok(this),
                }
            }
            CacheableBlockSpec::Earliest
            | CacheableBlockSpec::Safe
            | CacheableBlockSpec::Finalized => Err(UnresolvedBlockTagError),
        }
    }

    pub fn hash_log_filter_options(
        self,
        params: &CacheableLogFilterOptions<'_>,
    ) -> Result<Self, UnresolvedBlockTagError> {
        // Destructuring to make sure we get a compiler error here if the fields change.
        let CacheableLogFilterOptions {
            range,
            addresses,
            topics,
        } = params;

        let mut this = self
            .hash_log_filter_range(range)?
            .hash_u64(addresses.len() as u64);

        for address in addresses {
            this = this.hash_address(address);
        }

        this = this.hash_u64(topics.len() as u64);
        for options in topics {
            this = this.hash_u8(options.cache_key_variant());
            if let Some(options) = options {
                this = this.hash_u64(options.len() as u64);
                for option in options {
                    this = this.hash_b256(option);
                }
            }
        }

        Ok(this)
    }

    pub fn hash_log_filter_range(
        self,
        params: &CacheableLogFilterRange<'_>,
    ) -> Result<Self, UnresolvedBlockTagError> {
        let this = self.hash_u8(params.cache_key_variant());

        match params {
            CacheableLogFilterRange::Hash(block_hash) => Ok(this.hash_b256(block_hash)),
            CacheableLogFilterRange::Range {
                from_block,
                to_block,
            } => Ok(this
                .hash_block_spec(from_block)?
                .hash_block_spec(to_block)?),
        }
    }

    pub fn hash_reward_percentile(self, value: &RewardPercentile) -> Self {
        const RESOLUTION: f64 = 100.0;
        // `RewardPercentile` is an f64 in range [0, 100], so this is guaranteed not to
        // overflow.
        self.hash_u64((value.as_ref() * RESOLUTION).floor() as u64)
    }

    pub fn hash_reward_percentiles(self, value: &[RewardPercentile]) -> Self {
        let mut this = self.hash_u64(value.len() as u64);
        for v in value {
            this = this.hash_reward_percentile(v);
        }
        this
    }

    /// Finalizes the hash and returns it as a hex-encoded string.
    pub fn finalize(self) -> String {
        hex::encode(self.hasher.finalize_fixed())
    }
}
