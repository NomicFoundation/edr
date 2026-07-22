//! `DbBridge`: expose a revm@41 `Database` as a revm@38 `Database`, so
//! op-revm (revm@38) can execute over state owned by revm@41-typed EDR code.

use core::fmt;

use crate::convert;

pub struct DbBridge<DatabaseT> {
    pub inner: DatabaseT,
}

impl<DatabaseT> DbBridge<DatabaseT> {
    pub fn new(inner: DatabaseT) -> Self {
        Self { inner }
    }
}

/// Newtype so the revm@41 error can carry revm@38's `DBErrorMarker`.
#[derive(Debug)]
pub struct BridgeError<ErrorT>(pub ErrorT);

impl<ErrorT: fmt::Display> fmt::Display for BridgeError<ErrorT> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl<ErrorT: fmt::Debug + fmt::Display> core::error::Error for BridgeError<ErrorT> {}

impl<ErrorT> revm38::database_interface::DBErrorMarker for BridgeError<ErrorT> where
    ErrorT: fmt::Debug + fmt::Display + Send + Sync + 'static
{
}

impl<DatabaseT: revm41::Database> revm38::Database for DbBridge<DatabaseT> {
    type Error = BridgeError<DatabaseT::Error>;

    fn basic(
        &mut self,
        address: revm38::primitives::Address,
    ) -> Result<Option<revm38::state::AccountInfo>, Self::Error> {
        Ok(self
            .inner
            .basic(address)
            .map_err(BridgeError)?
            .map(convert::account_info_new_to_old))
    }

    fn code_by_hash(
        &mut self,
        code_hash: revm38::primitives::B256,
    ) -> Result<revm38::bytecode::Bytecode, Self::Error> {
        Ok(convert::bytecode_new_to_old(
            self.inner.code_by_hash(code_hash).map_err(BridgeError)?,
        ))
    }

    fn storage(
        &mut self,
        address: revm38::primitives::Address,
        index: revm38::primitives::StorageKey,
    ) -> Result<revm38::primitives::StorageValue, Self::Error> {
        self.inner.storage(address, index).map_err(BridgeError)
    }

    fn block_hash(&mut self, number: u64) -> Result<revm38::primitives::B256, Self::Error> {
        self.inner.block_hash(number).map_err(BridgeError)
    }
}
