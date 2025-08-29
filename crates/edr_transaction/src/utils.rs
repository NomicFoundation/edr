use alloy_rlp::{BufMut, Encodable};

/// RLP-encodes the provided value and prepends it with the provided ID.
pub fn enveloped<T: Encodable>(id: u8, v: &T, out: &mut dyn BufMut) {
    out.put_u8(id);
    v.encode(out);
}

/// Prepends the provided (RLP-encoded) bytes with the provided ID.
pub fn envelop_bytes(id: u8, bytes: &[u8]) -> Vec<u8> {
    let mut out = vec![0; 1 + bytes.len()];
    out[0] = id;
    out[1..].copy_from_slice(bytes);

    out
}
