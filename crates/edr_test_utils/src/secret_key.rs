use edr_eth::Address;
#[allow(deprecated)]
// This is test code, it's ok to use `DangerousSecretKeyStr`
use edr_signer::{public_key_to_address, DangerousSecretKeyStr};
pub use edr_signer::{SecretKey, SignatureError};

/// Converts a hex string to a secret key.
pub fn secret_key_from_str(secret_key: &str) -> Result<SecretKey, SignatureError> {
    // This is test code, it's ok to use `DangerousSecretKeyStr`
    #[allow(deprecated)]
    edr_signer::secret_key_from_str(DangerousSecretKeyStr(secret_key))
}

/// Converts a secret key in a hex string format to an address.
///
/// Note that this function is in `edr_test_utils` to restrict opportunities for
/// misuse. In production code there should be only one place where secret keys
/// are parsed from string to avoid potential leakage into logs and error
/// messages.
///
/// # Examples
///
/// ```
/// use edr_test_utils::secret_key::secret_key_to_address;
///
/// let secret_key = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";
///
/// let address = secret_key_to_address(secret_key).unwrap();
/// ```
pub fn secret_key_to_address(secret_key: &str) -> Result<Address, SignatureError> {
    // This is test code, it's ok to use `DangerousSecretKeyStr`
    #[allow(deprecated)]
    let secret_key = edr_signer::secret_key_from_str(DangerousSecretKeyStr(secret_key))?;
    Ok(public_key_to_address(secret_key.public_key()))
}

/// Converts a secret key to a 0x-prefixed hex string.
pub fn secret_key_to_str(secret_key: &SecretKey) -> String {
    format!("0x{}", hex::encode(secret_key.to_bytes().as_slice()))
}
