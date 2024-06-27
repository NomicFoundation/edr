use edr_eth::{utils::envelop_bytes, B256};
use revm::primitives::keccak256;

use super::Deposited;

impl Deposited {
    /// The type identifier for a deposited transaction.
    pub const TYPE: u8 = 0x7E;

    /// Returns the transaction's hash.
    pub fn transaction_hash(&self) -> &B256 {
        self.hash.get_or_init(|| {
            let encoded = alloy_rlp::encode(self);
            let enveloped = envelop_bytes(Self::TYPE, &encoded);

            keccak256(enveloped)
        })
    }
}

impl PartialEq for Deposited {
    fn eq(&self, other: &Self) -> bool {
        // Custom implementation of `PartialEq` to ignore the `hash` field.
        self.source_hash == other.source_hash
            && self.from == other.from
            && self.to == other.to
            && self.mint == other.mint
            && self.value == other.value
            && self.gas_limit == other.gas_limit
            && self.is_system_tx == other.is_system_tx
            && self.data == other.data
    }
}

#[cfg(test)]
mod tests {
    use std::{str::FromStr as _, sync::OnceLock};

    use edr_eth::{
        address,
        transaction::{SignedTransaction as _, TxKind},
        Bytes, U256,
    };
    use revm::primitives::b256;

    use super::*;
    use crate::transaction;

    #[test]
    fn transaction_hash() -> anyhow::Result<()> {
        // Source:
        // <https://optimism.blockscout.com/tx/0xcca2f31992022e3a833959c505de021285a7c5339c8d1b8ad75100074e1c6aea>
        const EXPECTED: B256 =
            b256!("cca2f31992022e3a833959c505de021285a7c5339c8d1b8ad75100074e1c6aea");

        let transaction = transaction::Signed::Deposited(Deposited {
            source_hash: b256!("8672083ef2a54fb901eab5c1366a77c1e2c421793467cf1ea7925f21282804bb"),
            from: address!("deaddeaddeaddeaddeaddeaddeaddeaddead0001"),
            to: TxKind::Call(address!("4200000000000000000000000000000000000015")),
            mint: 0,
            value: U256::ZERO,
            gas_limit: 1000000,
            is_system_tx: false,
            data: Bytes::from_str("440a5e2000000558000c5fc5000000000000000500000000667b267b000000000133c922000000000000000000000000000000000000000000000000000000017a2aaed000000000000000000000000000000000000000000000000000000000000000015b91c0cfdec539cc0ad4dadc77c6dd693f5116721d5cc39b73b85aefe7501b480000000000000000000000006887246668a3b87f54deb3b94ba47a6f63f32985")?,
            hash: OnceLock::new(),
        });

        assert_eq!(*transaction.transaction_hash(), EXPECTED);

        Ok(())
    }
}
