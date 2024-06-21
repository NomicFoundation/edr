use core::fmt::Debug;

use edr_eth::{
    transaction::{self, EthTransactionRequest},
    AccessListItem, Address, BlockSpec, BlockTag, Bytes, PreEip1898BlockSpec, SpecId, B256,
    MAX_INITCODE_SIZE, U256,
};
use edr_rpc_eth::CallRequest;

use crate::ProviderError;

/// Data used for validating a transaction complies with a [`SpecId`].
pub struct SpecValidationData<'data> {
    pub to: Option<&'data Address>,
    pub gas_price: Option<&'data U256>,
    pub max_fee_per_gas: Option<&'data U256>,
    pub max_priority_fee_per_gas: Option<&'data U256>,
    pub access_list: Option<&'data Vec<AccessListItem>>,
    pub blobs: Option<&'data Vec<Bytes>>,
    pub blob_hashes: Option<&'data Vec<B256>>,
}

impl<'data> From<&'data EthTransactionRequest> for SpecValidationData<'data> {
    fn from(value: &'data EthTransactionRequest) -> Self {
        Self {
            to: value.to.as_ref(),
            gas_price: value.gas_price.as_ref(),
            max_fee_per_gas: value.max_fee_per_gas.as_ref(),
            max_priority_fee_per_gas: value.max_priority_fee_per_gas.as_ref(),
            access_list: value.access_list.as_ref(),
            blobs: value.blobs.as_ref(),
            blob_hashes: value.blob_hashes.as_ref(),
        }
    }
}

impl<'data> From<&'data CallRequest> for SpecValidationData<'data> {
    fn from(value: &'data CallRequest) -> Self {
        Self {
            to: value.to.as_ref(),
            gas_price: value.gas_price.as_ref(),
            max_fee_per_gas: value.max_fee_per_gas.as_ref(),
            max_priority_fee_per_gas: value.max_priority_fee_per_gas.as_ref(),
            access_list: value.access_list.as_ref(),
            blobs: value.blobs.as_ref(),
            blob_hashes: value.blob_hashes.as_ref(),
        }
    }
}

impl<'data> From<&'data transaction::Signed> for SpecValidationData<'data> {
    fn from(value: &'data transaction::Signed) -> Self {
        match value {
            transaction::Signed::PreEip155Legacy(tx) => Self {
                to: tx.kind.to(),
                gas_price: Some(&tx.gas_price),
                max_fee_per_gas: None,
                max_priority_fee_per_gas: None,
                access_list: None,
                blobs: None,
                blob_hashes: None,
            },
            transaction::Signed::PostEip155Legacy(tx) => Self {
                to: tx.kind.to(),
                gas_price: Some(&tx.gas_price),
                max_fee_per_gas: None,
                max_priority_fee_per_gas: None,
                access_list: None,
                blobs: None,
                blob_hashes: None,
            },
            transaction::Signed::Eip2930(tx) => Self {
                to: tx.kind.to(),
                gas_price: Some(&tx.gas_price),
                max_fee_per_gas: None,
                max_priority_fee_per_gas: None,
                access_list: Some(tx.access_list.0.as_ref()),
                blobs: None,
                blob_hashes: None,
            },
            transaction::Signed::Eip1559(tx) => Self {
                to: tx.kind.to(),
                gas_price: None,
                max_fee_per_gas: Some(&tx.max_fee_per_gas),
                max_priority_fee_per_gas: Some(&tx.max_priority_fee_per_gas),
                access_list: Some(tx.access_list.0.as_ref()),
                blobs: None,
                blob_hashes: None,
            },
            transaction::Signed::Eip4844(tx) => Self {
                to: Some(&tx.to),
                gas_price: None,
                max_fee_per_gas: Some(&tx.max_fee_per_gas),
                max_priority_fee_per_gas: Some(&tx.max_priority_fee_per_gas),
                access_list: Some(tx.access_list.0.as_ref()),
                blobs: None,
                blob_hashes: Some(tx.blob_hashes.as_ref()),
            },
        }
    }
}

fn validate_transaction_spec<LoggerErrorT: Debug>(
    spec_id: SpecId,
    data: SpecValidationData<'_>,
) -> Result<(), ProviderError<LoggerErrorT>> {
    let SpecValidationData {
        to,
        gas_price,
        max_fee_per_gas,
        max_priority_fee_per_gas,
        access_list,
        blobs,
        blob_hashes,
    } = data;

    if spec_id < SpecId::BERLIN && access_list.is_some() {
        return Err(ProviderError::UnsupportedAccessListParameter {
            current_hardfork: spec_id,
            minimum_hardfork: SpecId::BERLIN,
        });
    }

    if spec_id < SpecId::LONDON && (max_fee_per_gas.is_some() || max_priority_fee_per_gas.is_some())
    {
        return Err(ProviderError::UnsupportedEIP1559Parameters {
            current_hardfork: spec_id,
            minimum_hardfork: SpecId::BERLIN,
        });
    }

    if spec_id < SpecId::CANCUN && (blobs.is_some() || blob_hashes.is_some()) {
        return Err(ProviderError::UnsupportedEIP4844Parameters {
            current_hardfork: spec_id,
            minimum_hardfork: SpecId::CANCUN,
        });
    }

    if gas_price.is_some() {
        if max_fee_per_gas.is_some() {
            return Err(ProviderError::InvalidTransactionInput(
                "Cannot send both gasPrice and maxFeePerGas params".to_string(),
            ));
        }

        if max_priority_fee_per_gas.is_some() {
            return Err(ProviderError::InvalidTransactionInput(
                "Cannot send both gasPrice and maxPriorityFeePerGas".to_string(),
            ));
        }

        if blobs.is_some() {
            return Err(ProviderError::InvalidTransactionInput(
                "Cannot send both gasPrice and blobs".to_string(),
            ));
        }

        if blob_hashes.is_some() {
            return Err(ProviderError::InvalidTransactionInput(
                "Cannot send both gasPrice and blobHashes".to_string(),
            ));
        }
    }

    if let Some(max_fee_per_gas) = max_fee_per_gas {
        if let Some(max_priority_fee_per_gas) = max_priority_fee_per_gas {
            if max_priority_fee_per_gas > max_fee_per_gas {
                return Err(ProviderError::InvalidTransactionInput(format!(
                    "maxPriorityFeePerGas ({max_priority_fee_per_gas}) is bigger than maxFeePerGas ({max_fee_per_gas})"),
                ));
            }
        }
    }

    if (blobs.is_some() || blob_hashes.is_some()) && to.is_none() {
        return Err(ProviderError::Eip4844TransactionMissingReceiver);
    }

    Ok(())
}

pub fn validate_call_request<LoggerErrorT: Debug>(
    spec_id: SpecId,
    call_request: &CallRequest,
    block_spec: &BlockSpec,
) -> Result<(), ProviderError<LoggerErrorT>> {
    validate_post_merge_block_tags(spec_id, block_spec)?;

    if call_request.blobs.is_some() | call_request.blob_hashes.is_some() {
        return Err(ProviderError::Eip4844CallRequestUnsupported);
    }

    validate_transaction_and_call_request(
        spec_id,
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

pub fn validate_transaction_and_call_request<'a, LoggerErrorT: Debug>(
    spec_id: SpecId,
    validation_data: impl Into<SpecValidationData<'a>>,
) -> Result<(), ProviderError<LoggerErrorT>> {
    validate_transaction_spec(spec_id, validation_data.into()).map_err(|err| match err {
        ProviderError::UnsupportedAccessListParameter {
            minimum_hardfork, ..
        } => ProviderError::InvalidArgument(format!(
            "\
Access list received but is not supported by the current hardfork. 

You can use them by running Hardhat Network with 'hardfork' {minimum_hardfork:?} or later.
        "
        )),
        err => err,
    })
}

pub fn validate_eip3860_max_initcode_size<LoggerErrorT: Debug>(
    spec_id: SpecId,
    allow_unlimited_contract_code_size: bool,
    to: Option<&Address>,
    data: &Bytes,
) -> Result<(), ProviderError<LoggerErrorT>> {
    if spec_id < SpecId::SHANGHAI || to.is_some() || allow_unlimited_contract_code_size {
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

pub fn validate_post_merge_block_tags<'a, LoggerErrorT: Debug>(
    hardfork: SpecId,
    block_spec: impl Into<ValidationBlockSpec<'a>>,
) -> Result<(), ProviderError<LoggerErrorT>> {
    let block_spec: ValidationBlockSpec<'a> = block_spec.into();

    if hardfork < SpecId::MERGE {
        match block_spec {
            ValidationBlockSpec::PreEip1898(PreEip1898BlockSpec::Tag(
                tag @ (BlockTag::Safe | BlockTag::Finalized),
            ))
            | ValidationBlockSpec::PostEip1898(BlockSpec::Tag(
                tag @ (BlockTag::Safe | BlockTag::Finalized),
            )) => {
                return Err(ProviderError::InvalidBlockTag {
                    block_tag: *tag,
                    spec: hardfork,
                });
            }
            _ => (),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_mixed_eip_1559_parameters(spec: SpecId) {
        let mixed_request = EthTransactionRequest {
            from: Address::ZERO,
            gas_price: Some(U256::ZERO),
            max_fee_per_gas: Some(U256::ZERO),
            ..EthTransactionRequest::default()
        };

        assert!(matches!(
            validate_transaction_spec::<()>(spec, (&mixed_request).into()),
            Err(ProviderError::InvalidTransactionInput(_))
        ));

        let mixed_request = EthTransactionRequest {
            from: Address::ZERO,
            gas_price: Some(U256::ZERO),
            max_priority_fee_per_gas: Some(U256::ZERO),
            ..EthTransactionRequest::default()
        };

        assert!(matches!(
            validate_transaction_spec::<()>(spec, (&mixed_request).into()),
            Err(ProviderError::InvalidTransactionInput(_))
        ));

        let request_with_too_low_max_fee = EthTransactionRequest {
            from: Address::ZERO,
            max_fee_per_gas: Some(U256::ZERO),
            max_priority_fee_per_gas: Some(U256::from(1u64)),
            ..EthTransactionRequest::default()
        };

        assert!(matches!(
            validate_transaction_spec::<()>(spec, (&request_with_too_low_max_fee).into()),
            Err(ProviderError::InvalidTransactionInput(_))
        ));
    }

    fn assert_unsupported_eip_1559_parameters(spec: SpecId) {
        let eip_1559_request = EthTransactionRequest {
            from: Address::ZERO,
            max_fee_per_gas: Some(U256::ZERO),
            ..EthTransactionRequest::default()
        };

        assert!(matches!(
            validate_transaction_spec::<()>(spec, (&eip_1559_request).into()),
            Err(ProviderError::UnsupportedEIP1559Parameters { .. })
        ));

        let eip_1559_request = EthTransactionRequest {
            from: Address::ZERO,
            max_priority_fee_per_gas: Some(U256::ZERO),
            ..EthTransactionRequest::default()
        };

        assert!(matches!(
            validate_transaction_spec::<()>(spec, (&eip_1559_request).into()),
            Err(ProviderError::UnsupportedEIP1559Parameters { .. })
        ));
    }

    fn assert_unsupported_eip_4844_parameters(spec: SpecId) {
        let eip_4844_request = EthTransactionRequest {
            from: Address::ZERO,
            blobs: Some(Vec::new()),
            ..EthTransactionRequest::default()
        };

        assert!(matches!(
            validate_transaction_spec::<()>(spec, (&eip_4844_request).into()),
            Err(ProviderError::UnsupportedEIP4844Parameters { .. })
        ));

        let eip_4844_request = EthTransactionRequest {
            from: Address::ZERO,
            blob_hashes: Some(Vec::new()),
            ..EthTransactionRequest::default()
        };

        assert!(matches!(
            validate_transaction_spec::<()>(spec, (&eip_4844_request).into()),
            Err(ProviderError::UnsupportedEIP4844Parameters { .. })
        ));
    }

    #[test]
    fn validate_transaction_spec_eip_155_invalid_inputs() {
        let eip155_spec = SpecId::MUIR_GLACIER;
        let valid_request = EthTransactionRequest {
            from: Address::ZERO,
            gas_price: Some(U256::ZERO),
            ..EthTransactionRequest::default()
        };

        assert!(validate_transaction_spec::<()>(eip155_spec, (&valid_request).into()).is_ok());

        let eip_2930_request = EthTransactionRequest {
            from: Address::ZERO,
            access_list: Some(Vec::new()),
            ..EthTransactionRequest::default()
        };

        assert!(matches!(
            validate_transaction_spec::<()>(eip155_spec, (&eip_2930_request).into()),
            Err(ProviderError::UnsupportedAccessListParameter { .. })
        ));

        assert_unsupported_eip_1559_parameters(eip155_spec);
        assert_unsupported_eip_4844_parameters(eip155_spec);
    }

    #[test]
    fn validate_transaction_spec_eip_2930_invalid_inputs() {
        let eip2930_spec = SpecId::BERLIN;
        let valid_request = EthTransactionRequest {
            from: Address::ZERO,
            gas_price: Some(U256::ZERO),
            access_list: Some(Vec::new()),
            ..EthTransactionRequest::default()
        };

        assert!(validate_transaction_spec::<()>(eip2930_spec, (&valid_request).into()).is_ok());

        assert_unsupported_eip_1559_parameters(eip2930_spec);
        assert_unsupported_eip_4844_parameters(eip2930_spec);
    }

    #[test]
    fn validate_transaction_spec_eip_1559_invalid_inputs() {
        let eip1559_spec = SpecId::LONDON;
        let valid_request = EthTransactionRequest {
            from: Address::ZERO,
            max_fee_per_gas: Some(U256::ZERO),
            max_priority_fee_per_gas: Some(U256::ZERO),
            access_list: Some(Vec::new()),
            ..EthTransactionRequest::default()
        };

        assert!(validate_transaction_spec::<()>(eip1559_spec, (&valid_request).into()).is_ok());

        assert_unsupported_eip_4844_parameters(eip1559_spec);
        assert_mixed_eip_1559_parameters(eip1559_spec);
    }

    #[test]
    fn validate_transaction_spec_eip_4844_invalid_inputs() {
        let eip4844_spec = SpecId::CANCUN;
        let valid_request = EthTransactionRequest {
            from: Address::ZERO,
            to: Some(Address::ZERO),
            max_fee_per_gas: Some(U256::ZERO),
            max_priority_fee_per_gas: Some(U256::ZERO),
            access_list: Some(Vec::new()),
            blobs: Some(Vec::new()),
            blob_hashes: Some(Vec::new()),
            ..EthTransactionRequest::default()
        };

        assert!(validate_transaction_spec::<()>(eip4844_spec, (&valid_request).into()).is_ok());
        assert_mixed_eip_1559_parameters(eip4844_spec);

        let mixed_request = EthTransactionRequest {
            from: Address::ZERO,
            gas_price: Some(U256::ZERO),
            blobs: Some(Vec::new()),
            ..EthTransactionRequest::default()
        };

        assert!(matches!(
            validate_transaction_spec::<()>(eip4844_spec, (&mixed_request).into()),
            Err(ProviderError::InvalidTransactionInput(_))
        ));

        let mixed_request = EthTransactionRequest {
            from: Address::ZERO,
            gas_price: Some(U256::ZERO),
            blob_hashes: Some(Vec::new()),
            ..EthTransactionRequest::default()
        };

        assert!(matches!(
            validate_transaction_spec::<()>(eip4844_spec, (&mixed_request).into()),
            Err(ProviderError::InvalidTransactionInput(_))
        ));

        let missing_receiver_request = EthTransactionRequest {
            from: Address::ZERO,
            blobs: Some(Vec::new()),
            ..EthTransactionRequest::default()
        };

        assert!(matches!(
            validate_transaction_spec::<()>(eip4844_spec, (&missing_receiver_request).into()),
            Err(ProviderError::Eip4844TransactionMissingReceiver)
        ));

        let missing_receiver_request = EthTransactionRequest {
            from: Address::ZERO,
            blob_hashes: Some(Vec::new()),
            ..EthTransactionRequest::default()
        };

        assert!(matches!(
            validate_transaction_spec::<()>(eip4844_spec, (&missing_receiver_request).into()),
            Err(ProviderError::Eip4844TransactionMissingReceiver)
        ));
    }
}
