//! Implementations of [`Crypto`](spec::Group::Crypto) Cheatcodes.

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
use p256::ecdsa::{
    signature::hazmat::PrehashSigner, Signature as P256Signature, SigningKey as P256SigningKey,
};
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
        let sig = sign(privateKey, digest)?;
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
        let sig = sign(privateKey, digest)?;
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

fn sign(private_key: &U256, digest: &B256) -> Result<alloy_primitives::Signature> {
    // The `ecrecover` precompile does not use EIP-155. No chain ID is needed.
    let wallet = parse_wallet(private_key)?;
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

pub(super) fn parse_wallet(private_key: &U256) -> Result<PrivateKeySigner> {
    parse_private_key(private_key).map(PrivateKeySigner::from)
}

#[cfg(test)]
mod tests {
    use alloy_primitives::{hex::FromHex, FixedBytes};
    use p256::ecdsa::signature::hazmat::PrehashVerifier;

    use super::*;

    #[test]
    fn test_sign_p256() {
        use p256::ecdsa::VerifyingKey;

        let pk_u256: U256 = "1".parse().unwrap();
        let signing_key = P256SigningKey::from_bytes(&pk_u256.to_be_bytes().into()).unwrap();
        let digest = FixedBytes::from_hex(
            "0x44acf6b7e36c1342c2c5897204fe09504e1e2efb1a900377dbc4e7a6a133ec56",
        )
        .unwrap();

        let result = sign_p256(&pk_u256, &digest).unwrap();
        let result_bytes: [u8; 64] = result.try_into().unwrap();
        let signature = P256Signature::from_bytes(&result_bytes.into()).unwrap();
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
            .unwrap();
        let digest = FixedBytes::from_hex(
            "0x54705ba3baafdbdfba8c5f9a70f7a89bee98d906b53e31074da7baecdc0da9ad",
        )
        .unwrap();
        let result = sign_p256(&pk, &digest);
        assert_eq!(
            result.err().unwrap().to_string(),
            "private key must be less than the NistP256 curve order (115792089210356248762697446949407573529996955224135760342422259061068512044369)"
        );
    }

    #[test]
    fn test_sign_p256_pk_0() {
        let digest = FixedBytes::from_hex(
            "0x54705ba3baafdbdfba8c5f9a70f7a89bee98d906b53e31074da7baecdc0da9ad",
        )
        .unwrap();
        let result = sign_p256(&U256::ZERO, &digest);
        assert_eq!(result.err().unwrap().to_string(), "private key cannot be 0");
    }
}
