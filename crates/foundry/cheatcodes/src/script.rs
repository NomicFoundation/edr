//! Implementations of the EIP-7702 delegation cheatcodes from the
//! [`Scripting`](spec::Group::Scripting) group.
//!
//! Solidity tests have no broadcast, so these only sign authorizations and
//! write the delegation designator to the authority's account. They match
//! Foundry's behavior in a test context.

use alloy_primitives::{Address, U256};
use alloy_rpc_types::Authorization;
use alloy_signer::SignerSync;
use alloy_sol_types::SolValue;
use foundry_evm_core::{
    backend::CheatcodeBackend,
    evm_context::{
        BlockEnvTr, ChainContextTr, EvmBuilderTrait, HardforkTr, TransactionEnvTr,
        TransactionErrorTrait,
    },
};
use revm::{
    bytecode::Bytecode,
    context::{result::HaltReasonTr, JournalTr},
    context_interface::transaction::SignedAuthorization,
    primitives::KECCAK_EMPTY,
};

use crate::{
    impl_is_pure_true, Cheatcode, CheatsCtxt, Result,
    Vm::{
        attachDelegation_0Call, attachDelegation_1Call, signAndAttachDelegation_0Call,
        signAndAttachDelegation_1Call, signAndAttachDelegation_2Call, signDelegation_0Call,
        signDelegation_1Call, signDelegation_2Call, SignedDelegation,
    },
};

impl_is_pure_true!(attachDelegation_0Call);
impl Cheatcode for attachDelegation_0Call {
    fn apply_stateful<
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
        ccx: &mut CheatsCtxt<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
            DatabaseT,
        >,
    ) -> Result {
        let Self { signedDelegation } = self;
        attach_delegation(ccx, signedDelegation, false)
    }
}

impl_is_pure_true!(attachDelegation_1Call);
impl Cheatcode for attachDelegation_1Call {
    fn apply_stateful<
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
        ccx: &mut CheatsCtxt<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
            DatabaseT,
        >,
    ) -> Result {
        let Self {
            signedDelegation,
            crossChain,
        } = self;
        attach_delegation(ccx, signedDelegation, *crossChain)
    }
}

impl_is_pure_true!(signDelegation_0Call);
impl Cheatcode for signDelegation_0Call {
    fn apply_stateful<
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
        ccx: &mut CheatsCtxt<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
            DatabaseT,
        >,
    ) -> Result {
        let Self {
            implementation,
            privateKey,
        } = *self;
        sign_delegation(ccx, &privateKey, implementation, None, false, false)
    }
}

impl_is_pure_true!(signDelegation_1Call);
impl Cheatcode for signDelegation_1Call {
    fn apply_stateful<
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
        ccx: &mut CheatsCtxt<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
            DatabaseT,
        >,
    ) -> Result {
        let Self {
            implementation,
            privateKey,
            nonce,
        } = *self;
        sign_delegation(ccx, &privateKey, implementation, Some(nonce), false, false)
    }
}

impl_is_pure_true!(signDelegation_2Call);
impl Cheatcode for signDelegation_2Call {
    fn apply_stateful<
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
        ccx: &mut CheatsCtxt<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
            DatabaseT,
        >,
    ) -> Result {
        let Self {
            implementation,
            privateKey,
            crossChain,
        } = *self;
        sign_delegation(ccx, &privateKey, implementation, None, crossChain, false)
    }
}

impl_is_pure_true!(signAndAttachDelegation_0Call);
impl Cheatcode for signAndAttachDelegation_0Call {
    fn apply_stateful<
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
        ccx: &mut CheatsCtxt<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
            DatabaseT,
        >,
    ) -> Result {
        let Self {
            implementation,
            privateKey,
        } = *self;
        sign_delegation(ccx, &privateKey, implementation, None, false, true)
    }
}

impl_is_pure_true!(signAndAttachDelegation_1Call);
impl Cheatcode for signAndAttachDelegation_1Call {
    fn apply_stateful<
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
        ccx: &mut CheatsCtxt<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
            DatabaseT,
        >,
    ) -> Result {
        let Self {
            implementation,
            privateKey,
            nonce,
        } = *self;
        sign_delegation(ccx, &privateKey, implementation, Some(nonce), false, true)
    }
}

impl_is_pure_true!(signAndAttachDelegation_2Call);
impl Cheatcode for signAndAttachDelegation_2Call {
    fn apply_stateful<
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
        ccx: &mut CheatsCtxt<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
            DatabaseT,
        >,
    ) -> Result {
        let Self {
            implementation,
            privateKey,
            crossChain,
        } = *self;
        sign_delegation(ccx, &privateKey, implementation, None, crossChain, true)
    }
}

/// Attaches a signed EIP-7702 delegation to its authority.
fn attach_delegation<
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
    ccx: &mut CheatsCtxt<
        BlockT,
        TxT,
        EvmBuilderT,
        HaltReasonT,
        HardforkT,
        TransactionErrorT,
        ChainContextT,
        DatabaseT,
    >,
    delegation: &SignedDelegation,
    cross_chain: bool,
) -> Result {
    let SignedDelegation {
        v,
        r,
        s,
        nonce,
        implementation,
    } = delegation;
    // A chain id of 0 makes the authorization valid on every chain. See
    // https://github.com/ethereum/EIPs/blob/master/EIPS/eip-7702.md#protection-from-malleability-cross-chain
    let chain_id = if cross_chain {
        U256::ZERO
    } else {
        U256::from(ccx.ecx.cfg.chain_id)
    };

    let auth = Authorization {
        address: *implementation,
        nonce: *nonce,
        chain_id,
    };
    let signed_auth = SignedAuthorization::new_unchecked(auth, *v, (*r).into(), (*s).into());
    write_delegation(ccx, signed_auth.clone())?;
    ccx.state.add_delegation(signed_auth);
    Ok(Vec::default())
}

/// Signs an EIP-7702 delegation and, if `attach` is set, attaches it.
///
/// Without an explicit `nonce`, the authority's next delegation nonce is used.
fn sign_delegation<
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
    ccx: &mut CheatsCtxt<
        BlockT,
        TxT,
        EvmBuilderT,
        HaltReasonT,
        HardforkT,
        TransactionErrorT,
        ChainContextT,
        DatabaseT,
    >,
    private_key: &U256,
    implementation: Address,
    nonce: Option<u64>,
    cross_chain: bool,
    attach: bool,
) -> Result {
    let signer = ccx.state.config.wallet_cache.get_or_derive(private_key)?;
    let nonce = if let Some(nonce) = nonce {
        nonce
    } else {
        let account_nonce = ccx
            .ecx
            .journaled_state
            .load_account(signer.address())?
            .info
            .nonce;
        next_delegation_nonce(
            &ccx.state.active_delegations,
            signer.address(),
            account_nonce,
        )
    };
    let chain_id = if cross_chain {
        U256::ZERO
    } else {
        U256::from(ccx.ecx.cfg.chain_id)
    };

    let auth = Authorization {
        address: implementation,
        nonce,
        chain_id,
    };
    let sig = signer.sign_hash_sync(&auth.signature_hash())?;
    if attach {
        let signed_auth =
            SignedAuthorization::new_unchecked(auth, u8::from(sig.v()), sig.r(), sig.s());
        write_delegation(ccx, signed_auth.clone())?;
        ccx.state.add_delegation(signed_auth);
    }
    Ok(SignedDelegation {
        v: u8::from(sig.v()),
        r: sig.r().into(),
        s: sig.s().into(),
        nonce,
        implementation,
    }
    .abi_encode())
}

/// Returns the nonce the next delegation of `authority` must use: one more than
/// its last attached delegation, or its account nonce if it has none.
///
/// Foundry also counts the nonce increment of a broadcast sent by the
/// authority. Solidity tests have no broadcast, so that case doesn't apply.
fn next_delegation_nonce(
    active_delegations: &[SignedAuthorization],
    authority: Address,
    account_nonce: u64,
) -> u64 {
    active_delegations
        .iter()
        .rfind(|auth| {
            auth.recover_authority()
                .is_ok_and(|recovered| recovered == authority)
        })
        .map_or(account_nonce, |auth| auth.nonce.saturating_add(1))
}

/// Checks the delegation's nonce and writes its delegation designator to the
/// authority's account.
fn write_delegation<
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
    ccx: &mut CheatsCtxt<
        BlockT,
        TxT,
        EvmBuilderT,
        HaltReasonT,
        HardforkT,
        TransactionErrorT,
        ChainContextT,
        DatabaseT,
    >,
    auth: SignedAuthorization,
) -> Result<()> {
    let authority = auth.recover_authority().map_err(|e| format!("{e}"))?;
    let account_nonce = ccx.ecx.journaled_state.load_account(authority)?.info.nonce;

    let expected_nonce =
        next_delegation_nonce(&ccx.state.active_delegations, authority, account_nonce);
    if expected_nonce != auth.nonce {
        return Err(format!(
            "invalid nonce for {authority:?}: expected {expected_nonce}, got {}",
            auth.nonce
        )
        .into());
    }

    if auth.address.is_zero() {
        // Delegating to the zero address clears the authority's code. See
        // https://github.com/ethereum/EIPs/blob/master/EIPS/eip-7702.md#behavior
        ccx.ecx
            .journaled_state
            .set_code_with_hash(authority, Bytecode::default(), KECCAK_EMPTY);
    } else {
        ccx.ecx
            .journaled_state
            .set_code(authority, Bytecode::new_eip7702(auth.address));
    }
    Ok(())
}
