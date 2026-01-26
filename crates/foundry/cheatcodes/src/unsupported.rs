//! Stub implementations for cheatcodes that are not supported in EDR.
//!
//! These cheatcodes exist in Foundry but are not implemented in EDR.
//! Calling them will result in an error explaining that the cheatcode is not
//! supported.

use foundry_evm_core::{
    backend::CheatcodeBackend,
    evm_context::{
        BlockEnvTr, ChainContextTr, EvmBuilderTrait, HardforkTr, TransactionEnvTr,
        TransactionErrorTrait,
    },
};
use revm::context::result::HaltReasonTr;

use crate::{
    impl_is_pure_false, impl_is_pure_true, Cheatcode, Cheatcodes, Result,
    Vm::{
        attachBlobCall, attachDelegation_0Call, attachDelegation_1Call, breakpoint_0Call,
        breakpoint_1Call, broadcastRawTransactionCall, broadcast_0Call, broadcast_1Call,
        broadcast_2Call, createWallet_0Call, createWallet_1Call, createWallet_2Call,
        deployCode_0Call, deployCode_1Call, deployCode_2Call, deployCode_3Call, deployCode_4Call,
        deployCode_5Call, deployCode_6Call, deployCode_7Call, deriveKey_0Call, deriveKey_1Call,
        deriveKey_2Call, deriveKey_3Call, eip712HashStruct_0Call, eip712HashStruct_1Call,
        eip712HashType_0Call, eip712HashType_1Call, eip712HashTypedDataCall,
        foundryVersionAtLeastCall, foundryVersionCmpCall, getArtifactPathByCodeCall,
        getArtifactPathByDeployedCodeCall, getBroadcastCall, getBroadcasts_0Call,
        getBroadcasts_1Call, getDeployment_0Call, getDeployment_1Call, getDeploymentsCall,
        getFoundryVersionCall, getWalletsCall, rememberKeyCall, rememberKeys_0Call,
        rememberKeys_1Call, signAndAttachDelegation_0Call, signAndAttachDelegation_1Call,
        signAndAttachDelegation_2Call, signDelegation_0Call, signDelegation_1Call,
        signDelegation_2Call, startBroadcast_0Call, startBroadcast_1Call, startBroadcast_2Call,
        stopBroadcastCall,
    },
};

/// Macro to implement unsupported cheatcodes that return an error when called.
///
/// Use `pure` for cheatcodes marked as `pure` in the cheatcode spec,
/// and `non_pure` for all others. This is done for correctness, it does not
/// affect behavior since the cheatcodes are unsupported.
macro_rules! impl_unsupported_cheatcode {
    (pure: $($call_type:ident => $cheatcode_name:literal),* $(,)?) => {
        $(
            impl_is_pure_true!($call_type);

            impl Cheatcode for $call_type {
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
                    bail!(concat!("cheatcode `", $cheatcode_name, "` is not supported"));
                }
            }
        )*
    };
    (non_pure: $($call_type:ident => $cheatcode_name:literal),* $(,)?) => {
        $(
            impl_is_pure_false!($call_type);

            impl Cheatcode for $call_type {
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
                    bail!(concat!("cheatcode `", $cheatcode_name, "` is not supported"));
                }
            }
        )*
    };
}

// Pure cheatcodes (marked as `pure` in the Foundry spec)
impl_unsupported_cheatcode! {
    pure:
    // Key derivation cheatcodes
    deriveKey_0Call => "deriveKey(string,uint32)",
    deriveKey_1Call => "deriveKey(string,string,uint32)",
    deriveKey_2Call => "deriveKey(string,uint32,string)",
    deriveKey_3Call => "deriveKey(string,string,uint32,string)",

    // EIP-712 cheatcodes
    eip712HashType_0Call => "eip712HashType(string,string)",
    eip712HashType_1Call => "eip712HashType(string,string,string)",
    eip712HashStruct_0Call => "eip712HashStruct(string,string,bytes)",
    eip712HashStruct_1Call => "eip712HashStruct(string,string,string,bytes)",
    eip712HashTypedDataCall => "eip712HashTypedData(string)",

    // Debugger cheatcodes
    breakpoint_0Call => "breakpoint(string)",
    breakpoint_1Call => "breakpoint(string,bool)",
}

// Non-pure cheatcodes (view or state-modifying)
impl_unsupported_cheatcode! {
    non_pure:
    // Broadcasting cheatcodes
    broadcast_0Call => "broadcast()",
    broadcast_1Call => "broadcast(address)",
    broadcast_2Call => "broadcast(uint256)",
    startBroadcast_0Call => "startBroadcast()",
    startBroadcast_1Call => "startBroadcast(address)",
    startBroadcast_2Call => "startBroadcast(uint256)",
    stopBroadcastCall => "stopBroadcast()",
    broadcastRawTransactionCall => "broadcastRawTransaction(bytes)",

    // EIP-7702 delegation cheatcodes
    signDelegation_0Call => "signDelegation(address,uint256)",
    signDelegation_1Call => "signDelegation(address,uint256,uint256)",
    signDelegation_2Call => "signDelegation(address,uint256,uint64)",
    attachDelegation_0Call => "attachDelegation((uint8,bytes32,bytes32,uint64,address))",
    attachDelegation_1Call => "attachDelegation((uint8,bytes32,bytes32,uint64,address),address)",
    signAndAttachDelegation_0Call => "signAndAttachDelegation(address,uint256)",
    signAndAttachDelegation_1Call => "signAndAttachDelegation(address,uint256,uint256)",
    signAndAttachDelegation_2Call => "signAndAttachDelegation(address,uint256,uint64)",

    // Blob cheatcodes (EIP-4844)
    attachBlobCall => "attachBlob(bytes)",

    // Wallet management cheatcodes
    getWalletsCall => "getWallets()",
    createWallet_0Call => "createWallet(string)",
    createWallet_1Call => "createWallet(uint256)",
    createWallet_2Call => "createWallet(uint256,string)",
    rememberKeyCall => "rememberKey(uint256)",
    rememberKeys_0Call => "rememberKeys(string,string,uint32)",
    rememberKeys_1Call => "rememberKeys(string,string,string,uint32)",

    // Artifact/deployment cheatcodes
    getArtifactPathByCodeCall => "getArtifactPathByCode(bytes)",
    getArtifactPathByDeployedCodeCall => "getArtifactPathByDeployedCode(bytes)",
    deployCode_0Call => "deployCode(string)",
    deployCode_1Call => "deployCode(string,bytes)",
    deployCode_2Call => "deployCode(string,uint256)",
    deployCode_3Call => "deployCode(string,bytes,uint256)",
    deployCode_4Call => "deployCode(string,address)",
    deployCode_5Call => "deployCode(string,bytes,address)",
    deployCode_6Call => "deployCode(string,uint256,address)",
    deployCode_7Call => "deployCode(string,bytes,uint256,address)",
    getBroadcastCall => "getBroadcast(string,uint64,uint8)",
    getBroadcasts_0Call => "getBroadcasts(string,uint64)",
    getBroadcasts_1Call => "getBroadcasts(string,uint64,uint8)",
    getDeployment_0Call => "getDeployment(string)",
    getDeployment_1Call => "getDeployment(string,uint64)",
    getDeploymentsCall => "getDeployments(string,uint64)",

    // Foundry version cheatcodes
    foundryVersionAtLeastCall => "foundryVersionAtLeast(uint256,uint256,uint256)",
    foundryVersionCmpCall => "foundryVersionCmp(uint256,uint256,uint256)",
    getFoundryVersionCall => "getFoundryVersion()",
}
