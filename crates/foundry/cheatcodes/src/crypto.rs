//! Implementations of [`Crypto`](spec::Group::Crypto) Cheatcodes.

use std::{fmt, num::NonZeroUsize};

use alloy_primitives::{B256, U256};
use alloy_signer::SignerSync;
use alloy_signer_local::PrivateKeySigner;
use alloy_sol_types::SolValue;
use foundry_evm_core::{
    backend::CheatcodeBackend,
    evm_context::{
        BlockEnvTr, ChainContextTr, EvmBuilderTrait, HardforkTr, TransactionEnvTr,
        TransactionErrorTrait,
    },
};
use k256::{
    ecdsa::SigningKey,
    elliptic_curve::{bigint::ArrayEncoding, sec1::ToEncodedPoint},
};
use lru::LruCache;
use p256::ecdsa::{
    signature::hazmat::PrehashSigner, Signature as P256Signature, SigningKey as P256SigningKey,
};
use parking_lot::Mutex;
use revm::context::result::HaltReasonTr;

use crate::{
    impl_is_pure_true, Cheatcode, Cheatcodes, Result,
    Vm::{publicKeyP256Call, signCall, signCompactCall, signP256Call},
};

impl_is_pure_true!(signCall);
impl Cheatcode for signCall {
    fn apply<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        TransactionErrorT: TransactionErrorTrait,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
        >,
    >(
        &self,
        state: &mut Cheatcodes<
            BlockT,
            TxT,
            ChainContextT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
        >,
    ) -> Result {
        let Self { privateKey, digest } = self;
        let sig = sign(&state.config.wallet_cache, privateKey, digest)?;
        Ok(encode_full_sig(sig))
    }
}

impl_is_pure_true!(signCompactCall);
impl Cheatcode for signCompactCall {
    fn apply<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        TransactionErrorT: TransactionErrorTrait,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
        >,
    >(
        &self,
        state: &mut Cheatcodes<
            BlockT,
            TxT,
            ChainContextT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
        >,
    ) -> Result {
        let Self { privateKey, digest } = self;
        let sig = sign(&state.config.wallet_cache, privateKey, digest)?;
        Ok(encode_compact_sig(sig))
    }
}

impl_is_pure_true!(signP256Call);
impl Cheatcode for signP256Call {
    fn apply<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        TransactionErrorT: TransactionErrorTrait,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
        >,
    >(
        &self,
        _state: &mut Cheatcodes<
            BlockT,
            TxT,
            ChainContextT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
        >,
    ) -> Result {
        let Self { privateKey, digest } = self;
        sign_p256(privateKey, digest)
    }
}

impl_is_pure_true!(publicKeyP256Call);
impl Cheatcode for publicKeyP256Call {
    fn apply<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        TransactionErrorT: TransactionErrorTrait,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
        >,
    >(
        &self,
        _state: &mut Cheatcodes<
            BlockT,
            TxT,
            ChainContextT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
        >,
    ) -> Result {
        let Self { privateKey } = self;
        let pub_key = parse_private_key_p256(privateKey)?
            .verifying_key()
            .as_affine()
            .to_encoded_point(false);
        let pub_key_x = U256::from_be_bytes((*pub_key.x().unwrap()).into());
        let pub_key_y = U256::from_be_bytes((*pub_key.y().unwrap()).into());

        Ok((pub_key_x, pub_key_y).abi_encode())
    }
}

fn encode_full_sig(sig: alloy_primitives::Signature) -> Vec<u8> {
    // Retrieve v, r and s from signature.
    let v = U256::from(u64::from(sig.v()) + 27);
    let r = B256::from(sig.r());
    let s = B256::from(sig.s());
    (v, r, s).abi_encode()
}

fn encode_compact_sig(sig: alloy_primitives::Signature) -> Vec<u8> {
    // Implement EIP-2098 compact signature.
    let r = B256::from(sig.r());
    let mut vs = sig.s();
    vs.set_bit(255, sig.v());
    (r, vs).abi_encode()
}

fn sign(
    wallet_cache: &WalletCache,
    private_key: &U256,
    digest: &B256,
) -> Result<alloy_primitives::Signature> {
    // The `ecrecover` precompile does not use EIP-155. No chain ID is needed.
    let wallet = wallet_cache.get_or_derive(private_key)?;
    let sig = wallet.sign_hash_sync(digest)?;
    debug_assert_eq!(sig.recover_address_from_prehash(digest)?, wallet.address());
    Ok(sig)
}

fn sign_p256(private_key: &U256, digest: &B256) -> Result {
    let signing_key = parse_private_key_p256(private_key)?;
    let signature: P256Signature = signing_key.sign_prehash(digest.as_slice())?;
    let signature = signature.normalize_s().unwrap_or(signature);
    let r_bytes: [u8; 32] = signature.r().to_bytes().into();
    let s_bytes: [u8; 32] = signature.s().to_bytes().into();

    Ok((r_bytes, s_bytes).abi_encode())
}

fn validate_private_key<C: ecdsa::PrimeCurve>(private_key: &U256) -> Result<()> {
    ensure!(*private_key != U256::ZERO, "private key cannot be 0");
    let order = U256::from_be_slice(&C::ORDER.to_be_byte_array());
    ensure!(
        *private_key < order,
        "private key must be less than the {curve:?} curve order ({order})",
        curve = C::default(),
    );

    Ok(())
}

fn parse_private_key(private_key: &U256) -> Result<SigningKey> {
    validate_private_key::<k256::Secp256k1>(private_key)?;
    Ok(SigningKey::from_bytes((&private_key.to_be_bytes()).into())?)
}

fn parse_private_key_p256(private_key: &U256) -> Result<P256SigningKey> {
    validate_private_key::<p256::NistP256>(private_key)?;
    Ok(P256SigningKey::from_bytes(
        (&private_key.to_be_bytes()).into(),
    )?)
}

/// Derives the wallet (public key and address) for `private_key`.
///
/// This performs a full secp256k1 scalar multiplication. Prefer
/// [`WalletCache::get_or_derive`] when the same key may be seen repeatedly.
fn parse_wallet(private_key: &U256) -> Result<PrivateKeySigner> {
    parse_private_key(private_key).map(PrivateKeySigner::from)
}

/// Bounded, thread-safe memoization of private key -> wallet derivations.
///
/// Private keys are test-only material, but they are still never exposed via
/// [`fmt::Debug`].
pub struct WalletCache {
    inner: Mutex<LruCache<U256, PrivateKeySigner>>,
}

impl WalletCache {
    /// The number of derivations retained by [`WalletCache::default`].
    pub const DEFAULT_CAPACITY: NonZeroUsize = NonZeroUsize::new(256).expect("literal is non-zero");

    /// Creates an empty cache that retains at most `capacity` derivations.
    pub fn new(capacity: NonZeroUsize) -> Self {
        Self {
            inner: Mutex::new(LruCache::new(capacity)),
        }
    }

    /// Returns the wallet for `private_key`, deriving and caching it on a
    /// miss.
    ///
    /// Invalid keys are rejected before any derivation and are never cached.
    pub fn get_or_derive(&self, private_key: &U256) -> Result<PrivateKeySigner> {
        if let Some(wallet) = self.inner.lock().get(private_key) {
            return Ok(wallet.clone());
        }

        // Derive outside the lock so concurrent callers don't serialize on the
        // expensive scalar multiplication. Two concurrent misses on the same key
        // derive twice and insert the same value, which is not an issue.
        let wallet = parse_wallet(private_key)?;
        self.inner.lock().put(*private_key, wallet.clone());
        Ok(wallet)
    }

    /// Returns the number of cached derivations.
    pub fn len(&self) -> usize {
        self.inner.lock().len()
    }

    /// Returns whether the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl fmt::Debug for WalletCache {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Deliberately omit the entries: they are private keys.
        f.debug_struct("WalletCache")
            .field("len", &self.len())
            .finish_non_exhaustive()
    }
}

impl Default for WalletCache {
    fn default() -> Self {
        Self::new(Self::DEFAULT_CAPACITY)
    }
}

#[cfg(test)]
mod tests {
    use alloy_primitives::{hex::FromHex, FixedBytes};
    use p256::ecdsa::signature::hazmat::PrehashVerifier;

    use super::*;

    #[test]
    fn test_sign_p256() {
        use p256::ecdsa::VerifyingKey;

        let pk_u256: U256 = "1".parse().expect("literal is a valid U256");
        let signing_key = P256SigningKey::from_bytes(&pk_u256.to_be_bytes().into())
            .expect("1 is a valid P-256 private key");
        let digest = FixedBytes::from_hex(
            "0x44acf6b7e36c1342c2c5897204fe09504e1e2efb1a900377dbc4e7a6a133ec56",
        )
        .expect("literal is a valid 32-byte hex digest");

        let result = sign_p256(&pk_u256, &digest).expect("signing with a valid key should succeed");
        let result_bytes: [u8; 64] = result
            .try_into()
            .expect("P-256 signature should be encoded as 64 bytes");
        let signature = P256Signature::from_bytes(&result_bytes.into())
            .expect("encoded bytes should be a valid P-256 signature");
        let verifying_key = VerifyingKey::from(&signing_key);
        assert!(verifying_key
            .verify_prehash(digest.as_slice(), &signature)
            .is_ok());
    }

    #[test]
    fn test_sign_p256_pk_too_large() {
        // max n from https://neuromancer.sk/std/secg/secp256r1
        let pk = "0xffffffff00000000ffffffffffffffffbce6faada7179e84f3b9cac2fc632551"
            .parse()
            .expect("literal is a valid U256");
        let digest = FixedBytes::from_hex(
            "0x54705ba3baafdbdfba8c5f9a70f7a89bee98d906b53e31074da7baecdc0da9ad",
        )
        .expect("literal is a valid 32-byte hex digest");
        let result = sign_p256(&pk, &digest);
        assert_eq!(
            result
                .expect_err("a key equal to the curve order should be rejected")
                .to_string(),
            "private key must be less than the NistP256 curve order (115792089210356248762697446949407573529996955224135760342422259061068512044369)"
        );
    }

    #[test]
    fn wallet_cache_matches_uncached_derivation() {
        let cache = WalletCache::default();

        // Standard test keys plus a "random-looking" one.
        let keys: [U256; 4] = [
            U256::from(1),
            U256::from(0xBEEF),
            U256::from(0xA11CE),
            "0x4c0883a69102937d6231471b5dbb6204fe5129617082792ae468d01a3f362318"
                .parse()
                .expect("literal is a valid U256"),
        ];

        for key in &keys {
            let expected = parse_wallet(key).expect("test key should be a valid private key");
            // First call populates the cache; the second one hits it.
            for _ in 0..2 {
                let cached = cache
                    .get_or_derive(key)
                    .expect("test key should be a valid private key");
                assert_eq!(cached.address(), expected.address());
                assert_eq!(
                    cached.credential().verifying_key(),
                    expected.credential().verifying_key()
                );
            }
        }

        assert_eq!(cache.len(), keys.len());
    }

    #[test]
    fn wallet_cache_signatures_match_uncached() {
        let cache = WalletCache::default();
        let key = U256::from(0xBEEF);
        let digest = FixedBytes::from_hex(
            "0x44acf6b7e36c1342c2c5897204fe09504e1e2efb1a900377dbc4e7a6a133ec56",
        )
        .expect("literal is a valid 32-byte hex digest");

        let expected = parse_wallet(&key)
            .expect("test key should be a valid private key")
            .sign_hash_sync(&digest)
            .expect("signing with a valid key should succeed");
        // Sign twice so the second signature comes from a cached wallet.
        for _ in 0..2 {
            let actual =
                sign(&cache, &key, &digest).expect("signing with a valid key should succeed");
            assert_eq!(actual, expected);
        }
    }

    #[test]
    fn wallet_cache_rejects_and_does_not_cache_invalid_keys() {
        let cache = WalletCache::default();

        let err = cache
            .get_or_derive(&U256::ZERO)
            .expect_err("the zero key should be rejected");
        assert_eq!(err.to_string(), "private key cannot be 0");

        let err = cache
            .get_or_derive(&U256::MAX)
            .expect_err("a key above the curve order should be rejected");
        assert!(
            err.to_string()
                .starts_with("private key must be less than the Secp256k1 curve order"),
            "{err}"
        );

        assert!(cache.is_empty());
    }

    #[test]
    fn wallet_cache_is_bounded() {
        let cache = WalletCache::new(NonZeroUsize::new(2).expect("literal is non-zero"));

        for key in 1..=3u64 {
            cache
                .get_or_derive(&U256::from(key))
                .expect("test key should be a valid private key");
        }

        assert_eq!(cache.len(), 2);
        // Evicted entries are re-derived correctly.
        assert_eq!(
            cache
                .get_or_derive(&U256::from(1))
                .expect("test key should be a valid private key")
                .address(),
            parse_wallet(&U256::from(1))
                .expect("test key should be a valid private key")
                .address()
        );
    }

    #[test]
    fn wallet_cache_debug_does_not_leak_keys() {
        let cache = WalletCache::default();
        let key: U256 = "0x4c0883a69102937d6231471b5dbb6204fe5129617082792ae468d01a3f362318"
            .parse()
            .expect("literal is a valid U256");
        let wallet = cache
            .get_or_derive(&key)
            .expect("test key should be a valid private key");

        let debug = format!("{cache:?}");
        assert!(!debug.contains("4c0883a6"), "{debug}");
        assert!(!debug.contains(&format!("{key}")), "{debug}");
        assert!(
            !debug.contains(&format!("{:?}", wallet.address())),
            "{debug}"
        );
    }

    #[test]
    fn test_sign_p256_pk_0() {
        let digest = FixedBytes::from_hex(
            "0x54705ba3baafdbdfba8c5f9a70f7a89bee98d906b53e31074da7baecdc0da9ad",
        )
        .expect("literal is a valid 32-byte hex digest");
        let result = sign_p256(&U256::ZERO, &digest);
        assert_eq!(
            result
                .expect_err("the zero key should be rejected")
                .to_string(),
            "private key cannot be 0"
        );
    }
}
