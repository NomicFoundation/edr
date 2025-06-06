//! Implementations of [`Utils`](crate::Group::Utils) cheatcodes.

use alloy_primitives::{B256, U256};
use alloy_signer::SignerSync;
use alloy_signer_local::PrivateKeySigner;
use alloy_sol_types::SolValue;
use foundry_evm_core::{
    constants::DEFAULT_CREATE2_DEPLOYER,
    evm_context::{
        BlockEnvTr, ChainContextTr, EvmBuilderTrait, HardforkTr, TransactionEnvTr,
        TransactionErrorTrait,
    },
};
use k256::{ecdsa::SigningKey, elliptic_curve::Curve, Secp256k1};
use p256::ecdsa::{signature::hazmat::PrehashSigner, Signature, SigningKey as P256SigningKey};
use revm::context::result::HaltReasonTr;

use crate::{
    ens::namehash,
    impl_is_pure_true, Cheatcode, Cheatcodes, Result,
    Vm::{
        computeCreate2Address_0Call, computeCreate2Address_1Call, computeCreateAddressCall,
        ensNamehashCall, getLabelCall, labelCall,
    },
};
impl_is_pure_true!(labelCall);
impl Cheatcode for labelCall {
    fn apply<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        ChainContextT: ChainContextTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        TransactionErrorT: TransactionErrorTrait,
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
        let Self { account, newLabel } = self;
        state.labels.insert(*account, newLabel.clone());
        Ok(Vec::default())
    }
}

impl_is_pure_true!(getLabelCall);
impl Cheatcode for getLabelCall {
    fn apply<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        ChainContextT: ChainContextTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        TransactionErrorT: TransactionErrorTrait,
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
        let Self { account } = self;
        Ok(match state.labels.get(account) {
            Some(label) => label.abi_encode(),
            None => format!("unlabeled:{account}").abi_encode(),
        })
    }
}

impl_is_pure_true!(computeCreateAddressCall);
impl Cheatcode for computeCreateAddressCall {
    fn apply<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        ChainContextT: ChainContextTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        TransactionErrorT: TransactionErrorTrait,
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
        let Self { nonce, deployer } = self;
        ensure!(
            *nonce <= U256::from(u64::MAX),
            "nonce must be less than 2^64 - 1"
        );
        Ok(deployer.create(nonce.to()).abi_encode())
    }
}

impl_is_pure_true!(computeCreate2Address_0Call);
impl Cheatcode for computeCreate2Address_0Call {
    fn apply<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        ChainContextT: ChainContextTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        TransactionErrorT: TransactionErrorTrait,
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
        let Self {
            salt,
            initCodeHash,
            deployer,
        } = self;
        Ok(deployer.create2(salt, initCodeHash).abi_encode())
    }
}

impl_is_pure_true!(computeCreate2Address_1Call);
impl Cheatcode for computeCreate2Address_1Call {
    fn apply<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        ChainContextT: ChainContextTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        TransactionErrorT: TransactionErrorTrait,
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
        let Self { salt, initCodeHash } = self;
        Ok(DEFAULT_CREATE2_DEPLOYER
            .create2(salt, initCodeHash)
            .abi_encode())
    }
}

impl_is_pure_true!(ensNamehashCall);
impl Cheatcode for ensNamehashCall {
    fn apply<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        ChainContextT: ChainContextTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        TransactionErrorT: TransactionErrorTrait,
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
        let Self { name } = self;
        Ok(namehash(name).abi_encode())
    }
}

fn encode_vrs(sig: alloy_primitives::PrimitiveSignature) -> Vec<u8> {
    // Retrieve v, r and s from signature.
    let v = U256::from(u64::from(sig.v()) + 27);
    let r = B256::from(sig.r());
    let s = B256::from(sig.s());
    (v, r, s).abi_encode()
}

pub(super) fn sign(private_key: &U256, digest: &B256) -> Result {
    // The `ecrecover` precompile does not use EIP-155. No chain ID is needed.
    let wallet = parse_wallet(private_key)?;
    let sig = wallet.sign_hash_sync(digest)?;
    debug_assert_eq!(sig.recover_address_from_prehash(digest)?, wallet.address());
    Ok(encode_vrs(sig))
}

pub(super) fn sign_p256(private_key: &U256, digest: &B256) -> Result {
    ensure!(*private_key != U256::ZERO, "private key cannot be 0");
    let n = U256::from_limbs(*p256::NistP256::ORDER.as_words());
    ensure!(
        *private_key < n,
        format!(
            "private key must be less than the secp256r1 curve order ({})",
            n
        ),
    );
    let bytes = private_key.to_be_bytes();
    let signing_key = P256SigningKey::from_bytes((&bytes).into())?;
    let signature: Signature = signing_key.sign_prehash(digest.as_slice())?;
    let r_bytes: [u8; 32] = signature.r().to_bytes().into();
    let s_bytes: [u8; 32] = signature.s().to_bytes().into();

    Ok((r_bytes, s_bytes).abi_encode())
}

pub(super) fn parse_private_key(private_key: &U256) -> Result<SigningKey> {
    ensure!(*private_key != U256::ZERO, "private key cannot be 0");
    ensure!(
        *private_key < U256::from_limbs(*Secp256k1::ORDER.as_words()),
        "private key must be less than the secp256k1 curve order \
         (115792089237316195423570985008687907852837564279074904382605163141518161494337)",
    );
    let bytes = private_key.to_be_bytes();
    SigningKey::from_bytes((&bytes).into()).map_err(Into::into)
}

pub(super) fn parse_wallet(private_key: &U256) -> Result<PrivateKeySigner> {
    parse_private_key(private_key).map(PrivateKeySigner::from)
}

#[cfg(test)]
mod tests {

    use alloy_primitives::FixedBytes;
    use hex::FromHex;
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
        let signature = Signature::from_bytes(&result_bytes.into()).unwrap();
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
        assert_eq!(result.err().unwrap().to_string(), "private key must be less than the secp256r1 curve order (115792089210356248762697446949407573529996955224135760342422259061068512044369)");
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
