use edr_eth::{
    eips::{eip2930, eip7702},
    l1,
    transaction::{pooled::PooledTransaction, ExecutableTransaction},
    Address, Blob, BlockSpec, BlockTag, Bytes, PreEip1898BlockSpec, B256, MAX_INITCODE_SIZE,
};
use edr_evm::{spec::RuntimeSpec, transaction};
use edr_rpc_eth::{CallRequest, TransactionRequest};

use crate::{
    data::ProviderData, error::ProviderErrorForChainSpec, spec::HardforkValidationData,
    time::TimeSinceEpoch, ProviderError, SyncProviderSpec,
};

impl HardforkValidationData for TransactionRequest {
    fn to(&self) -> Option<&Address> {
        self.to.as_ref()
    }

    fn gas_price(&self) -> Option<&u128> {
        self.gas_price.as_ref()
    }

    fn max_fee_per_gas(&self) -> Option<&u128> {
        self.max_fee_per_gas.as_ref()
    }

    fn max_priority_fee_per_gas(&self) -> Option<&u128> {
        self.max_priority_fee_per_gas.as_ref()
    }

    fn access_list(&self) -> Option<&Vec<eip2930::AccessListItem>> {
        self.access_list.as_ref()
    }

    fn blobs(&self) -> Option<&Vec<Blob>> {
        self.blobs.as_ref()
    }

    fn blob_hashes(&self) -> Option<&Vec<B256>> {
        self.blob_hashes.as_ref()
    }

    fn authorization_list(&self) -> Option<&Vec<eip7702::SignedAuthorization>> {
        self.authorization_list.as_ref()
    }
}

impl HardforkValidationData for CallRequest {
    fn to(&self) -> Option<&Address> {
        self.to.as_ref()
    }

    fn gas_price(&self) -> Option<&u128> {
        self.gas_price.as_ref()
    }

    fn max_fee_per_gas(&self) -> Option<&u128> {
        self.max_fee_per_gas.as_ref()
    }

    fn max_priority_fee_per_gas(&self) -> Option<&u128> {
        self.max_priority_fee_per_gas.as_ref()
    }

    fn access_list(&self) -> Option<&Vec<eip2930::AccessListItem>> {
        self.access_list.as_ref()
    }

    fn blobs(&self) -> Option<&Vec<Blob>> {
        self.blobs.as_ref()
    }

    fn blob_hashes(&self) -> Option<&Vec<B256>> {
        self.blob_hashes.as_ref()
    }

    fn authorization_list(&self) -> Option<&Vec<eip7702::SignedAuthorization>> {
        self.authorization_list.as_ref()
    }
}

impl HardforkValidationData for PooledTransaction {
    fn to(&self) -> Option<&Address> {
        Some(self.caller())
    }

    fn gas_price(&self) -> Option<&u128> {
        match self {
            PooledTransaction::PreEip155Legacy(tx) => Some(&tx.gas_price),
            PooledTransaction::PostEip155Legacy(tx) => Some(&tx.gas_price),
            PooledTransaction::Eip2930(tx) => Some(&tx.gas_price),
            PooledTransaction::Eip1559(_)
            | PooledTransaction::Eip4844(_)
            | PooledTransaction::Eip7702(_) => None,
        }
    }

    fn max_fee_per_gas(&self) -> Option<&u128> {
        ExecutableTransaction::max_fee_per_gas(self)
    }

    fn max_priority_fee_per_gas(&self) -> Option<&u128> {
        ExecutableTransaction::max_priority_fee_per_gas(self)
    }

    fn access_list(&self) -> Option<&Vec<eip2930::AccessListItem>> {
        match self {
            PooledTransaction::PreEip155Legacy(_) | PooledTransaction::PostEip155Legacy(_) => None,
            PooledTransaction::Eip2930(tx) => Some(tx.access_list.0.as_ref()),
            PooledTransaction::Eip1559(tx) => Some(tx.access_list.0.as_ref()),
            PooledTransaction::Eip4844(tx) => Some(&tx.payload().access_list),
            PooledTransaction::Eip7702(tx) => Some(tx.access_list.0.as_ref()),
        }
    }

    fn blobs(&self) -> Option<&Vec<Blob>> {
        match self {
            PooledTransaction::Eip4844(tx) => Some(tx.blobs_ref()),
            _ => None,
        }
    }

    fn blob_hashes(&self) -> Option<&Vec<B256>> {
        match self {
            PooledTransaction::Eip4844(tx) => Some(&tx.payload().blob_hashes),
            _ => None,
        }
    }

    fn authorization_list(&self) -> Option<&Vec<eip7702::SignedAuthorization>> {
        match self {
            PooledTransaction::Eip7702(tx) => Some(tx.authorization_list.as_ref()),
            _ => None,
        }
    }
}

/// Validates a `TransactionRequest` against the provided `ProviderData`.
pub fn validate_send_transaction_request<
    ChainSpecT: SyncProviderSpec<TimerT>,
    TimerT: Clone + TimeSinceEpoch,
>(
    data: &ProviderData<ChainSpecT, TimerT>,
    request: &TransactionRequest,
) -> Result<(), ProviderErrorForChainSpec<ChainSpecT>> {
    if let Some(chain_id) = request.chain_id {
        let expected = data.chain_id();
        if chain_id != expected {
            return Err(ProviderError::InvalidChainId {
                expected,
                actual: chain_id,
            });
        }
    }

    if let Some(request_data) = &request.data {
        validate_eip3860_max_initcode_size::<ChainSpecT>(
            data.evm_spec_id(),
            data.allow_unlimited_initcode_size(),
            request.to.as_ref(),
            request_data,
        )?;
    }

    if request.blob_hashes.is_some() || request.blobs.is_some() {
        return Err(ProviderError::Eip4844TransactionUnsupported);
    }

    if let Some(transaction_type) = request.transaction_type {
        if transaction_type == u8::from(transaction::Type::Eip4844) {
            return Err(ProviderError::Eip4844TransactionUnsupported);
        }
    }

    validate_transaction_and_call_request::<ChainSpecT>(data.hardfork(), request).map_err(|err| match err {
        ProviderError::UnsupportedEIP1559Parameters {
            minimum_hardfork, ..
        } => ProviderError::InvalidArgument(format!("\
EIP-1559 style fee params (maxFeePerGas or maxPriorityFeePerGas) received but they are not supported by the current hardfork.

You can use them by running Hardhat Network with 'hardfork' {minimum_hardfork:?} or later.
        ")),
        err => err,
    })
}

fn validate_transaction_spec<ChainSpecT: RuntimeSpec>(
    spec_id: l1::SpecId,
    value: &impl HardforkValidationData,
) -> Result<(), ProviderErrorForChainSpec<ChainSpecT>> {
    if spec_id < l1::SpecId::BERLIN && value.access_list().is_some() {
        return Err(ProviderError::UnsupportedAccessListParameter {
            current_hardfork: spec_id,
            minimum_hardfork: l1::SpecId::BERLIN,
        });
    }

    if spec_id < l1::SpecId::LONDON
        && (value.max_fee_per_gas().is_some() || value.max_priority_fee_per_gas().is_some())
    {
        return Err(ProviderError::UnsupportedEIP1559Parameters {
            current_hardfork: spec_id,
            minimum_hardfork: l1::SpecId::BERLIN,
        });
    }

    if spec_id < l1::SpecId::CANCUN && (value.blobs().is_some() || value.blob_hashes().is_some()) {
        return Err(ProviderError::UnsupportedEIP4844Parameters {
            current_hardfork: spec_id,
            minimum_hardfork: l1::SpecId::CANCUN,
        });
    }

    if spec_id < l1::SpecId::PRAGUE && value.authorization_list().is_some() {
        return Err(ProviderError::UnsupportedEip7702Parameters {
            current_hardfork: spec_id,
        });
    }

    if value.gas_price().is_some() {
        if value.max_fee_per_gas().is_some() {
            return Err(ProviderError::InvalidTransactionInput(
                "Cannot send both gasPrice and maxFeePerGas params".to_string(),
            ));
        }

        if value.max_priority_fee_per_gas().is_some() {
            return Err(ProviderError::InvalidTransactionInput(
                "Cannot send both gasPrice and maxPriorityFeePerGas".to_string(),
            ));
        }

        if value.blobs().is_some() {
            return Err(ProviderError::InvalidTransactionInput(
                "Cannot send both gasPrice and blobs".to_string(),
            ));
        }

        if value.blob_hashes().is_some() {
            return Err(ProviderError::InvalidTransactionInput(
                "Cannot send both gasPrice and blobHashes".to_string(),
            ));
        }

        if value.authorization_list().is_some() {
            return Err(ProviderError::InvalidTransactionInput(
                "Cannot send both gasPrice and authorizationList".to_string(),
            ));
        }
    }

    if let Some(max_fee_per_gas) = value.max_fee_per_gas() {
        if let Some(max_priority_fee_per_gas) = value.max_priority_fee_per_gas() {
            if max_priority_fee_per_gas > max_fee_per_gas {
                return Err(ProviderError::InvalidTransactionInput(format!(
                    "maxPriorityFeePerGas ({max_priority_fee_per_gas}) is bigger than maxFeePerGas ({max_fee_per_gas})"
                )));
            }
        }
    }

    if (value.blobs().is_some() || value.blob_hashes().is_some()) && value.to().is_none() {
        return Err(ProviderError::Eip4844TransactionMissingReceiver);
    }

    if let Some(authorization_list) = value.authorization_list() {
        if value.to().is_none() {
            return Err(ProviderError::Eip7702TransactionMissingReceiver);
        }

        if authorization_list.is_empty() {
            return Err(ProviderError::Eip7702TransactionWithoutAuthorizations);
        }
    }

    Ok(())
}

/// Validates a `CallRequest` and `BlockSpec` against the provided hardfork.
pub fn validate_call_request<ChainSpecT: RuntimeSpec>(
    hardfork: ChainSpecT::Hardfork,
    call_request: &CallRequest,
    block_spec: &BlockSpec,
) -> Result<(), ProviderErrorForChainSpec<ChainSpecT>> {
    validate_post_merge_block_tags::<ChainSpecT>(hardfork, block_spec)?;

    if call_request.blobs.is_some() | call_request.blob_hashes.is_some() {
        return Err(ProviderError::Eip4844CallRequestUnsupported);
    }

    validate_transaction_and_call_request::<ChainSpecT>(
        hardfork,
        call_request
    ).map_err(|err| match err {
        ProviderError::UnsupportedEIP1559Parameters {
            minimum_hardfork, ..
        } => ProviderError::InvalidArgument(format!("\
EIP-1559 style fee params (maxFeePerGas or maxPriorityFeePerGas) received but they are not supported by the current hardfork.

You can use them by running Hardhat Network with 'hardfork' {minimum_hardfork:?} or later.
        ")),
        err => err,
    })
}

pub(crate) fn validate_transaction_and_call_request<ChainSpecT: RuntimeSpec>(
    hardfork: ChainSpecT::Hardfork,
    validation_data: &impl HardforkValidationData,
) -> Result<(), ProviderErrorForChainSpec<ChainSpecT>> {
    validate_transaction_spec::<ChainSpecT>(hardfork.into(), validation_data).map_err(|err| {
        match err {
            ProviderError::UnsupportedAccessListParameter {
                minimum_hardfork, ..
            } => ProviderError::InvalidArgument(format!(
                "\
Access list received but is not supported by the current hardfork. 

You can use them by running Hardhat Network with 'hardfork' {minimum_hardfork:?} or later.
        "
            )),
            err => err,
        }
    })
}

pub(crate) fn validate_eip3860_max_initcode_size<ChainSpecT: RuntimeSpec>(
    spec_id: l1::SpecId,
    allow_unlimited_contract_code_size: bool,
    to: Option<&Address>,
    data: &Bytes,
) -> Result<(), ProviderErrorForChainSpec<ChainSpecT>> {
    if spec_id < l1::SpecId::SHANGHAI || to.is_some() || allow_unlimited_contract_code_size {
        return Ok(());
    }

    if data.len() > MAX_INITCODE_SIZE {
        return Err(ProviderError::InvalidArgument(format!("
Trying to send a deployment transaction whose init code length is {}. The max length allowed by EIP-3860 is {}.

Enable the 'allowUnlimitedContractSize' option to allow init codes of any length.", data.len(), MAX_INITCODE_SIZE)));
    }

    Ok(())
}

pub enum ValidationBlockSpec<'a> {
    PreEip1898(&'a PreEip1898BlockSpec),
    PostEip1898(&'a BlockSpec),
}

impl<'a> From<&'a PreEip1898BlockSpec> for ValidationBlockSpec<'a> {
    fn from(value: &'a PreEip1898BlockSpec) -> Self {
        Self::PreEip1898(value)
    }
}

impl<'a> From<&'a BlockSpec> for ValidationBlockSpec<'a> {
    fn from(value: &'a BlockSpec) -> Self {
        Self::PostEip1898(value)
    }
}

impl<'a> From<ValidationBlockSpec<'a>> for BlockSpec {
    fn from(value: ValidationBlockSpec<'a>) -> Self {
        match value {
            ValidationBlockSpec::PreEip1898(PreEip1898BlockSpec::Number(block_number)) => {
                BlockSpec::Number(*block_number)
            }
            ValidationBlockSpec::PreEip1898(PreEip1898BlockSpec::Tag(block_tag)) => {
                BlockSpec::Tag(*block_tag)
            }
            ValidationBlockSpec::PostEip1898(block_spec) => block_spec.clone(),
        }
    }
}

pub(crate) fn validate_post_merge_block_tags<'a, ChainSpecT: RuntimeSpec>(
    hardfork: ChainSpecT::Hardfork,
    block_spec: impl Into<ValidationBlockSpec<'a>>,
) -> Result<(), ProviderErrorForChainSpec<ChainSpecT>> {
    let block_spec: ValidationBlockSpec<'a> = block_spec.into();

    if hardfork.into() < l1::SpecId::MERGE {
        match block_spec {
            ValidationBlockSpec::PreEip1898(PreEip1898BlockSpec::Tag(
                tag @ (BlockTag::Safe | BlockTag::Finalized),
            ))
            | ValidationBlockSpec::PostEip1898(BlockSpec::Tag(
                tag @ (BlockTag::Safe | BlockTag::Finalized),
            )) => {
                return Err(ProviderError::InvalidBlockTag {
                    block_tag: *tag,
                    hardfork,
                });
            }
            _ => (),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use edr_eth::{l1::L1ChainSpec, U256};

    use super::*;

    fn assert_mixed_eip_1559_parameters(spec: l1::SpecId) {
        let mixed_request = TransactionRequest {
            from: Address::ZERO,
            gas_price: Some(0),
            max_fee_per_gas: Some(0),
            ..TransactionRequest::default()
        };

        assert!(matches!(
            validate_transaction_spec::<L1ChainSpec>(spec, &mixed_request),
            Err(ProviderError::InvalidTransactionInput(_))
        ));

        let mixed_request = TransactionRequest {
            from: Address::ZERO,
            gas_price: Some(0),
            max_priority_fee_per_gas: Some(0),
            ..TransactionRequest::default()
        };

        assert!(matches!(
            validate_transaction_spec::<L1ChainSpec>(spec, &mixed_request),
            Err(ProviderError::InvalidTransactionInput(_))
        ));

        let request_with_too_low_max_fee = TransactionRequest {
            from: Address::ZERO,
            max_fee_per_gas: Some(0),
            max_priority_fee_per_gas: Some(1),
            ..TransactionRequest::default()
        };

        assert!(matches!(
            validate_transaction_spec::<L1ChainSpec>(spec, &request_with_too_low_max_fee),
            Err(ProviderError::InvalidTransactionInput(_))
        ));
    }

    fn assert_unsupported_eip_1559_parameters(spec: l1::SpecId) {
        let eip_1559_request = TransactionRequest {
            from: Address::ZERO,
            max_fee_per_gas: Some(0),
            ..TransactionRequest::default()
        };

        assert!(matches!(
            validate_transaction_spec::<L1ChainSpec>(spec, &eip_1559_request),
            Err(ProviderError::UnsupportedEIP1559Parameters { .. })
        ));

        let eip_1559_request = TransactionRequest {
            from: Address::ZERO,
            max_priority_fee_per_gas: Some(0),
            ..TransactionRequest::default()
        };

        assert!(matches!(
            validate_transaction_spec::<L1ChainSpec>(spec, &eip_1559_request),
            Err(ProviderError::UnsupportedEIP1559Parameters { .. })
        ));
    }

    fn assert_unsupported_eip_4844_parameters(spec: l1::SpecId) {
        let eip_4844_request = TransactionRequest {
            from: Address::ZERO,
            blobs: Some(Vec::new()),
            ..TransactionRequest::default()
        };

        assert!(matches!(
            validate_transaction_spec::<L1ChainSpec>(spec, &eip_4844_request),
            Err(ProviderError::UnsupportedEIP4844Parameters { .. })
        ));

        let eip_4844_request = TransactionRequest {
            from: Address::ZERO,
            blob_hashes: Some(Vec::new()),
            ..TransactionRequest::default()
        };

        assert!(matches!(
            validate_transaction_spec::<L1ChainSpec>(spec, &eip_4844_request),
            Err(ProviderError::UnsupportedEIP4844Parameters { .. })
        ));
    }

    fn assert_unsuporrted_eip_7702_parameters(spec: l1::SpecId) {
        let eip_7702_request = TransactionRequest {
            from: Address::ZERO,
            authorization_list: Some(Vec::new()),
            ..TransactionRequest::default()
        };

        assert!(matches!(
            validate_transaction_spec::<L1ChainSpec>(spec, &eip_7702_request),
            Err(ProviderError::UnsupportedEip7702Parameters { .. })
        ));
    }

    #[test]
    fn validate_transaction_spec_eip_155_invalid_inputs() {
        let eip155_spec = l1::SpecId::MUIR_GLACIER;
        let valid_request = TransactionRequest {
            from: Address::ZERO,
            gas_price: Some(0),
            ..TransactionRequest::default()
        };

        assert!(validate_transaction_spec::<L1ChainSpec>(eip155_spec, &valid_request).is_ok());

        let eip_2930_request = TransactionRequest {
            from: Address::ZERO,
            access_list: Some(Vec::new()),
            ..TransactionRequest::default()
        };

        assert!(matches!(
            validate_transaction_spec::<L1ChainSpec>(eip155_spec, &eip_2930_request),
            Err(ProviderError::UnsupportedAccessListParameter { .. })
        ));

        assert_unsupported_eip_1559_parameters(eip155_spec);
        assert_unsupported_eip_4844_parameters(eip155_spec);
        assert_unsuporrted_eip_7702_parameters(eip155_spec);
    }

    #[test]
    fn validate_transaction_spec_eip_2930_invalid_inputs() {
        let eip2930_spec = l1::SpecId::BERLIN;
        let valid_request = TransactionRequest {
            from: Address::ZERO,
            gas_price: Some(0),
            access_list: Some(Vec::new()),
            ..TransactionRequest::default()
        };

        assert!(validate_transaction_spec::<L1ChainSpec>(eip2930_spec, &valid_request).is_ok());

        assert_unsupported_eip_1559_parameters(eip2930_spec);
        assert_unsupported_eip_4844_parameters(eip2930_spec);
        assert_unsuporrted_eip_7702_parameters(eip2930_spec);
    }

    #[test]
    fn validate_transaction_spec_eip_1559_invalid_inputs() {
        let eip1559_spec = l1::SpecId::LONDON;
        let valid_request = TransactionRequest {
            from: Address::ZERO,
            max_fee_per_gas: Some(0),
            max_priority_fee_per_gas: Some(0),
            access_list: Some(Vec::new()),
            ..TransactionRequest::default()
        };

        assert!(validate_transaction_spec::<L1ChainSpec>(eip1559_spec, &valid_request).is_ok());

        assert_unsupported_eip_4844_parameters(eip1559_spec);
        assert_unsuporrted_eip_7702_parameters(eip1559_spec);
        assert_mixed_eip_1559_parameters(eip1559_spec);
    }

    #[test]
    fn validate_transaction_spec_eip_4844_invalid_inputs() {
        let eip4844_spec = l1::SpecId::CANCUN;
        let valid_request = TransactionRequest {
            from: Address::ZERO,
            to: Some(Address::ZERO),
            max_fee_per_gas: Some(0),
            max_priority_fee_per_gas: Some(0),
            access_list: Some(Vec::new()),
            blobs: Some(Vec::new()),
            blob_hashes: Some(Vec::new()),
            ..TransactionRequest::default()
        };

        assert!(validate_transaction_spec::<L1ChainSpec>(eip4844_spec, &valid_request).is_ok());
        assert_mixed_eip_1559_parameters(eip4844_spec);

        let mixed_request = TransactionRequest {
            from: Address::ZERO,
            gas_price: Some(0),
            blobs: Some(Vec::new()),
            ..TransactionRequest::default()
        };

        assert!(matches!(
            validate_transaction_spec::<L1ChainSpec>(eip4844_spec, &mixed_request),
            Err(ProviderError::InvalidTransactionInput(_))
        ));

        let mixed_request = TransactionRequest {
            from: Address::ZERO,
            gas_price: Some(0),
            blob_hashes: Some(Vec::new()),
            ..TransactionRequest::default()
        };

        assert!(matches!(
            validate_transaction_spec::<L1ChainSpec>(eip4844_spec, &mixed_request),
            Err(ProviderError::InvalidTransactionInput(_))
        ));

        let missing_receiver_request = TransactionRequest {
            from: Address::ZERO,
            blobs: Some(Vec::new()),
            ..TransactionRequest::default()
        };

        assert!(matches!(
            validate_transaction_spec::<L1ChainSpec>(eip4844_spec, &missing_receiver_request),
            Err(ProviderError::Eip4844TransactionMissingReceiver)
        ));

        let missing_receiver_request = TransactionRequest {
            from: Address::ZERO,
            blob_hashes: Some(Vec::new()),
            ..TransactionRequest::default()
        };

        assert!(matches!(
            validate_transaction_spec::<L1ChainSpec>(eip4844_spec, &missing_receiver_request),
            Err(ProviderError::Eip4844TransactionMissingReceiver)
        ));

        assert_unsuporrted_eip_7702_parameters(eip4844_spec);
    }

    #[test]
    fn validate_transaction_spec_eip_7702_invalid_inputs() {
        let eip7702_spec = l1::SpecId::PRAGUE;
        let valid_request = TransactionRequest {
            from: Address::ZERO,
            to: Some(Address::ZERO),
            max_fee_per_gas: Some(0),
            max_priority_fee_per_gas: Some(0),
            access_list: Some(Vec::new()),
            authorization_list: Some(vec![eip7702::SignedAuthorization::new_unchecked(
                eip7702::Authorization {
                    chain_id: U256::ZERO,
                    address: Address::ZERO,
                    nonce: 1,
                },
                0,
                U256::ZERO,
                U256::ZERO,
            )]),
            ..TransactionRequest::default()
        };

        assert!(validate_transaction_spec::<L1ChainSpec>(eip7702_spec, &valid_request).is_ok());
        assert_mixed_eip_1559_parameters(eip7702_spec);

        let mixed_request = TransactionRequest {
            from: Address::ZERO,
            gas_price: Some(0),
            authorization_list: Some(Vec::new()),
            ..TransactionRequest::default()
        };

        assert!(matches!(
            validate_transaction_spec::<L1ChainSpec>(eip7702_spec, &mixed_request),
            Err(ProviderError::InvalidTransactionInput(_))
        ));

        let missing_receiver_request = TransactionRequest {
            from: Address::ZERO,
            authorization_list: Some(Vec::new()),
            ..TransactionRequest::default()
        };

        assert!(matches!(
            validate_transaction_spec::<L1ChainSpec>(eip7702_spec, &missing_receiver_request),
            Err(ProviderError::Eip7702TransactionMissingReceiver)
        ));

        let empty_authorization_list_request = TransactionRequest {
            from: Address::ZERO,
            to: Some(Address::ZERO),
            authorization_list: Some(Vec::new()),
            ..TransactionRequest::default()
        };

        assert!(matches!(
            validate_transaction_spec::<L1ChainSpec>(
                eip7702_spec,
                &empty_authorization_list_request
            ),
            Err(ProviderError::Eip7702TransactionWithoutAuthorizations)
        ));
    }
}
