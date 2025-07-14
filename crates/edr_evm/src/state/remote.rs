mod cached;

use std::sync::Arc;

pub use cached::CachedRemoteState;
use derive_where::derive_where;
use edr_eth::{
    account::AccountInfo, Address, BlockSpec, Bytecode, PreEip1898BlockSpec, B256, U256,
};
use edr_rpc_eth::{
    client::{EthRpcClient, RpcClientError},
    spec::RpcSpec,
};
use tokio::runtime;

use super::{State, StateError};
use crate::{spec::RuntimeSpec, EthRpcBlock as _};

/// A state backed by a remote Ethereum node
#[derive_where(Debug)]
pub struct RemoteState<ChainSpecT: RpcSpec> {
    client: Arc<EthRpcClient<ChainSpecT>>,
    runtime: runtime::Handle,
    block_number: u64,
}

impl<ChainSpecT: RpcSpec> RemoteState<ChainSpecT> {
    /// Construct a new instance using an RPC client for a remote Ethereum node
    /// and a block number from which data will be pulled.
    pub fn new(
        runtime: runtime::Handle,
        client: Arc<EthRpcClient<ChainSpecT>>,
        block_number: u64,
    ) -> Self {
        Self {
            client,
            runtime,
            block_number,
        }
    }

    /// Retrieves the current block number
    pub fn block_number(&self) -> u64 {
        self.block_number
    }

    /// Whether the current state is cacheable based on the block number.
    pub fn is_cacheable(&self) -> Result<bool, StateError> {
        Ok(tokio::task::block_in_place(move || {
            self.runtime
                .block_on(self.client.is_cacheable_block_number(self.block_number))
        })?)
    }

    /// Sets the block number used for calls to the remote Ethereum node.
    pub fn set_block_number(&mut self, block_number: u64) {
        self.block_number = block_number;
    }
}

impl<ChainSpecT: RuntimeSpec> RemoteState<ChainSpecT> {
    /// Retrieve the state root of the given block, if it exists.
    pub fn state_root(&self, block_number: u64) -> Result<Option<B256>, RpcClientError> {
        Ok(tokio::task::block_in_place(move || {
            self.runtime.block_on(
                self.client
                    .get_block_by_number(PreEip1898BlockSpec::Number(block_number)),
            )
        })?
        .map(|block| *block.state_root()))
    }
}

impl<ChainSpecT: RpcSpec> State for RemoteState<ChainSpecT> {
    type Error = StateError;

    #[cfg_attr(feature = "tracing", tracing::instrument(level = "trace", skip(self)))]
    fn basic(&self, address: Address) -> Result<Option<AccountInfo>, Self::Error> {
        Ok(Some(tokio::task::block_in_place(move || {
            self.runtime
                .block_on(
                    self.client
                        .get_account_info(address, Some(BlockSpec::Number(self.block_number))),
                )
                .map_err(StateError::Remote)
        })?))
    }

    #[cfg_attr(feature = "tracing", tracing::instrument(skip(self)))]
    fn code_by_hash(&self, code_hash: B256) -> Result<Bytecode, Self::Error> {
        Err(StateError::InvalidCodeHash(code_hash))
    }

    #[cfg_attr(feature = "tracing", tracing::instrument(skip(self)))]
    fn storage(&self, address: Address, index: U256) -> Result<U256, Self::Error> {
        Ok(tokio::task::block_in_place(move || {
            self.runtime
                .block_on(self.client.get_storage_at(
                    address,
                    index,
                    Some(BlockSpec::Number(self.block_number)),
                ))
                .map_err(StateError::Remote)
        })?
        .unwrap_or(U256::ZERO))
    }
}
