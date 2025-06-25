use edr_solidity_tests::{backend::Predeploy, revm::state::AccountInfo};
use napi::bindgen_prelude::{BigInt, Uint8Array};
use napi_derive::napi;

use crate::cast::TryCast;

/// Specification of overrides for an account and its storage.
#[napi(object)]
pub struct AccountOverride {
    /// The account's address
    pub address: Uint8Array,
    /// If present, the overwriting balance.
    pub balance: Option<BigInt>,
    /// If present, the overwriting nonce.
    pub nonce: Option<BigInt>,
    /// If present, the overwriting code.
    pub code: Option<Uint8Array>,
    /// BEWARE: This field is not supported yet. See <https://github.com/NomicFoundation/edr/issues/911>
    ///
    /// If present, the overwriting storage.
    pub storage: Option<Vec<StorageSlot>>,
}

impl TryFrom<AccountOverride> for (edr_eth::Address, edr_provider::AccountOverride) {
    type Error = napi::Error;

    fn try_from(value: AccountOverride) -> Result<Self, Self::Error> {
        let AccountOverride {
            address,
            balance,
            nonce,
            code,
            storage,
        } = value;
        let storage = storage
            .map(|storage| {
                storage
                    .into_iter()
                    .map(|StorageSlot { index, value }| {
                        let value = value.try_cast()?;
                        let slot = edr_evm::state::EvmStorageSlot::new(value);

                        let index: edr_eth::U256 = index.try_cast()?;
                        Ok((index, slot))
                    })
                    .collect::<napi::Result<_>>()
            })
            .transpose()?;

        let account_override = edr_provider::AccountOverride {
            balance: balance.map(TryCast::try_cast).transpose()?,
            nonce: nonce.map(TryCast::try_cast).transpose()?,
            code: code.map(TryCast::try_cast).transpose()?,
            storage,
        };

        let address: edr_eth::Address = address.try_cast()?;

        Ok((address, account_override))
    }
}

impl TryFrom<AccountOverride> for Predeploy {
    type Error = napi::Error;

    fn try_from(value: AccountOverride) -> Result<Self, Self::Error> {
        let (address, account_override) = value.try_into()?;

        macro_rules! predeploy_error {
            ($field:expr) => {
                || {
                    napi::Error::from_reason(format!(
                        "Predeploy with address '{address}' must have {field}",
                        field = $field
                    ))
                }
            };
        }

        let storage = account_override
            .storage
            .ok_or_else(predeploy_error!("storage"))?;
        let balance = account_override
            .balance
            .ok_or_else(predeploy_error!("balance"))?;
        let nonce = account_override
            .nonce
            .ok_or_else(predeploy_error!("nonce"))?;
        let code = account_override.code.ok_or_else(predeploy_error!("code"))?;

        if code.is_empty() {
            return Err(napi::Error::from_reason(
                "Predeploy with address '{address}' must have non-empty code",
            ));
        }
        let code_hash = code.hash_slow();

        let account_info = AccountInfo {
            balance,
            nonce,
            code_hash,
            code: Some(code),
        };

        Ok(Self {
            address,
            account_info,
            storage,
        })
    }
}

/// A description of a storage slot's state.
#[napi(object)]
pub struct StorageSlot {
    /// The storage slot's index
    pub index: BigInt,
    /// The storage slot's value
    pub value: BigInt,
}
