use edr_primitives::{address, bytes, Address, Bytecode, Bytes};
use edr_state_api::account::AccountInfo;

/// Address of the deterministic `CREATE2` factory, required from the Amsterdam
/// hardfork by EIP-7997.
pub const DETERMINISTIC_FACTORY_ADDRESS: Address =
    address!("0x4e59b44847b379578588920cA78FbF26c0B4956C");

/// Runtime code of the deterministic `CREATE2` factory, as specified by
/// EIP-7997.
pub const DETERMINISTIC_FACTORY_BYTECODE: Bytes = bytes!(
    "0x7fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe03601600081602082378035828234f58015156039578182fd5b8082525050506014600cf3"
);

pub(crate) fn deterministic_factory_contract() -> AccountInfo {
    let code = Bytecode::new_raw(DETERMINISTIC_FACTORY_BYTECODE);

    AccountInfo {
        nonce: 1,
        code_hash: code.hash_slow(),
        code: Some(code),
        ..AccountInfo::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // EIP-7997 requires a nonzero nonce alongside the runtime code; genesis
    // insertion uses 1.
    #[test]
    fn factory_account_has_nonce_one_and_eip_code() {
        let account = deterministic_factory_contract();

        assert_eq!(account.nonce, 1);
        assert_eq!(
            account.code.expect("factory has code").original_bytes(),
            DETERMINISTIC_FACTORY_BYTECODE
        );
    }
}
