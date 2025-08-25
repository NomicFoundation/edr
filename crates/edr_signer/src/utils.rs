use revm_primitives::{keccak256, B256};

/// Hash a message according to EIP-191.
///
/// The data is a UTF-8 encoded string and will be enveloped as follows:
/// `"\x19Ethereum Signed Message:\n" + message.length + message` and hashed
/// using keccak256.
pub fn hash_message<S>(message: S) -> B256
where
    S: AsRef<[u8]>,
{
    const PREFIX: &str = "\x19Ethereum Signed Message:\n";

    let message = message.as_ref();

    let mut eth_message = format!("{}{}", PREFIX, message.len()).into_bytes();
    eth_message.extend_from_slice(message);

    keccak256(&eth_message)
}
