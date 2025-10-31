//! A state implementation that retrieves data from a remote Ethereum node via
//! JSON-RPC.
mod cached;

use std::sync::Arc;

use derive_where::derive_where;
use edr_eth::{BlockSpec, PreEip1898BlockSpec};
use edr_primitives::{Address, Bytecode, B256, KECCAK_EMPTY, U256};
use edr_rpc_eth::client::{EthRpcClient, RpcClientError};
use edr_rpc_spec::{RpcBlockChainSpec, RpcEthBlock};
use edr_state_api::{account::AccountInfo, State, StateError};
use serde::{de::DeserializeOwned, Serialize};
use tokio::runtime;

pub use self::cached::CachedRemoteState;

/// A state backed by a remote Ethereum node
#[derive_where(Debug)]
pub struct RemoteState<
    RpcBlockChainSpecT: RpcBlockChainSpec,
    RpcReceiptT: DeserializeOwned + Serialize,
    RpcTransactionT: DeserializeOwned + Serialize,
> {
    client: Arc<EthRpcClient<RpcBlockChainSpecT, RpcReceiptT, RpcTransactionT>>,
    runtime: runtime::Handle,
    block_number: u64,
}

impl<
        RpcBlockT: RpcBlockChainSpec,
        RpcReceiptT: DeserializeOwned + Serialize,
        RpcTransactionT: DeserializeOwned + Serialize,
    > RemoteState<RpcBlockT, RpcReceiptT, RpcTransactionT>
{
    /// Construct a new instance using an RPC client for a remote Ethereum node
    /// and a block number from which data will be pulled.
    pub fn new(
        runtime: runtime::Handle,
        client: Arc<EthRpcClient<RpcBlockT, RpcReceiptT, RpcTransactionT>>,
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

    /// Sets the block number used for calls to the remote Ethereum node.
    pub fn set_block_number(&mut self, block_number: u64) {
        self.block_number = block_number;
    }
}

impl<
        RpcBlockT: RpcBlockChainSpec<RpcBlock<B256>: RpcEthBlock>,
        RpcReceiptT: DeserializeOwned + Serialize,
        RpcTransactionT: DeserializeOwned + Serialize,
    > RemoteState<RpcBlockT, RpcReceiptT, RpcTransactionT>
{
    /// Whether the current state is cacheable based on the block number.
    pub fn is_cacheable(&self) -> Result<bool, StateError> {
        Ok(tokio::task::block_in_place(move || {
            self.runtime
                .block_on(self.client.is_cacheable_block_number(self.block_number))
        })?)
    }

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

impl<
        RpcBlockT: RpcBlockChainSpec<RpcBlock<B256>: RpcEthBlock>,
        RpcReceiptT: DeserializeOwned + Serialize,
        RpcTransactionT: DeserializeOwned + Serialize,
    > State for RemoteState<RpcBlockT, RpcReceiptT, RpcTransactionT>
{
    type Error = StateError;

    #[cfg_attr(feature = "tracing", tracing::instrument(level = "trace", skip(self)))]
    fn basic(&self, address: Address) -> Result<Option<AccountInfo>, Self::Error> {
        let account_info = tokio::task::block_in_place(move || {
            self.runtime
                .block_on(
                    self.client
                        .get_account_info(address, Some(BlockSpec::Number(self.block_number))),
                )
                .map_err(StateError::Remote)
        })?;

        // If the account is empty, we must return `None` to signify non-existence.
        // See <https://github.com/bluealloy/revm/pull/3097>
        // Using bitwise AND to avoid short-circuiting
        let account_info = if (account_info.code_hash == KECCAK_EMPTY)
            & (account_info.nonce == 0)
            & (account_info.balance == U256::ZERO)
        {
            None
        } else {
            Some(account_info)
        };

        Ok(account_info)
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

#[cfg(all(test, feature = "test-remote"))]
mod tests {
    use std::str::FromStr;

    use edr_chain_l1::L1ChainSpec;
    use edr_rpc_eth::client::EthRpcClientForChainSpec;
    use tokio::runtime;

    use super::*;

    #[tokio::test(flavor = "multi_thread")]
    async fn basic_success() {
        let tempdir = tempfile::tempdir().expect("can create tempdir");

        let alchemy_url = std::env::var_os("ALCHEMY_URL")
            .expect("ALCHEMY_URL environment variable not defined")
            .into_string()
            .expect("couldn't convert OsString into a String");

        let rpc_client = EthRpcClientForChainSpec::<L1ChainSpec>::new(
            &alchemy_url,
            tempdir.path().to_path_buf(),
            None,
        )
        .expect("url ok");

        let dai_address = Address::from_str("0x6b175474e89094c44da98b954eedeac495271d0f")
            .expect("failed to parse address");

        let runtime = runtime::Handle::current();

        let account_info: AccountInfo = RemoteState::new(runtime, Arc::new(rpc_client), 16643427)
            .basic(dai_address)
            .expect("should succeed")
            .unwrap();

        assert_eq!(account_info.balance, U256::from(0));
        assert_eq!(account_info.nonce, 1);
    }
}
