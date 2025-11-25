use std::{
    collections::BTreeMap,
    net::{Ipv6Addr, SocketAddr, TcpListener, TcpStream},
    str::FromStr as _,
    sync::mpsc::{self, TryRecvError},
};

use edr_block_api::{GenesisBlockFactory as _, GenesisBlockOptions};
use edr_block_header::BlockConfig;
use edr_blockchain_api::{BlockchainMetadata as _, StateAtBlock as _};
use edr_chain_l1::{
    L1ChainSpec, L1SignedTransaction, L1_BASE_FEE_PARAMS, L1_MIN_ETHASH_DIFFICULTY,
};
use edr_debugger_bytecode::BytecodeDebugger;
use edr_debugger_tcp::create_tcp_debugger;
use edr_evm::dry_run_with_inspector;
use edr_evm_spec::{config::EvmConfig, BlockEnv};
use edr_primitives::{Address, Bytes, HashMap, TxKind, B256, U256};
use edr_provider::spec::LocalBlockchainForChainSpec;
use edr_signer::{public_key_to_address, SecretKey};
use edr_state_api::{State, StateDiff, StateError};
use edr_test_blockchain::deploy_contract;
use edr_test_utils::secret_key::secret_key_from_str;

const CHAIN_ID: u64 = 31337;

const INCREMENT_DEPLOYED_BYTECODE: &str =
    include_str!("../../../../../data/deployed_bytecode/increment.in");

fn call_inc_by_transaction(
    state: &dyn State<Error = StateError>,
    call_inc_address: Address,
    secret_key: &SecretKey,
) -> anyhow::Result<L1SignedTransaction> {
    // > cast sig 'incBy(uint)'
    const SELECTOR: &str = "0x70119d06";

    // > cast calldata 'function incBy(uint)' 1
    // 0x70119d060000000000000000000000000000000000000000000000000000000000000001
    let calldata = format!("{SELECTOR}{increment:0>64x}", increment = U256::ZERO);

    let caller = public_key_to_address(secret_key.public_key());

    let nonce = state.basic(caller)?.map_or(0, |info| info.nonce);
    let request = edr_chain_l1::request::Eip1559 {
        chain_id: CHAIN_ID,
        nonce,
        max_priority_fee_per_gas: 1_000,
        max_fee_per_gas: 1_000,
        gas_limit: 1_000_000,
        kind: TxKind::Call(call_inc_address),
        value: U256::ZERO,
        input: Bytes::from_str(&calldata).expect("Failed to parse hex"),
        access_list: Vec::new(),
    };

    let signed = request.sign(secret_key)?;

    Ok(signed.into())
}

pub struct TcpDebuggerFixture {
    debugger_handle: std::thread::JoinHandle<anyhow::Result<()>>,
    listener_handle: std::thread::JoinHandle<anyhow::Result<()>>,
    next_request_id: i64,
    tcp_stream: TcpStream,
}

impl TcpDebuggerFixture {
    pub fn new() -> anyhow::Result<Self> {
        let block_config = BlockConfig {
            base_fee_params: &L1_BASE_FEE_PARAMS,
            hardfork: edr_chain_l1::Hardfork::CANCUN,
            min_ethash_difficulty: L1_MIN_ETHASH_DIFFICULTY,
        };

        let genesis_diff = StateDiff::default();
        let genesis_block = L1ChainSpec::genesis_block(
            genesis_diff.clone(),
            block_config.clone(),
            GenesisBlockOptions {
                mix_hash: Some(B256::random()),
                ..GenesisBlockOptions::default()
            },
        )?;

        let blockchain = LocalBlockchainForChainSpec::<L1ChainSpec>::new(
            genesis_block,
            genesis_diff,
            CHAIN_ID,
            block_config,
        )?;

        let mut state = blockchain.state_at_block_number(0, &BTreeMap::new())?;
        let secret_key = secret_key_from_str(edr_defaults::SECRET_KEYS[0])?;

        let call_inc_address = deploy_contract(
            &blockchain,
            &mut state,
            Bytes::from_str(INCREMENT_DEPLOYED_BYTECODE).expect("Invalid bytecode"),
            &secret_key,
        )
        .expect("Failed to deploy");

        let transaction = call_inc_by_transaction(&state, call_inc_address, &secret_key)?;

        let evm_config = EvmConfig::with_chain_id(blockchain.chain_id());
        let block_env = BlockEnv {
            number: U256::from(1),
            ..BlockEnv::default()
        };

        let server = TcpListener::bind(SocketAddr::new(Ipv6Addr::LOCALHOST.into(), 0))
            .expect("Failed to bind server");
        let server_address = server.local_addr().expect("Failed to get local address");

        let debugger =
            create_tcp_debugger(server_address, true).expect("Failed to connect to server");

        let hardfork = blockchain.hardfork();

        let debugger_handle = std::thread::spawn(move || -> anyhow::Result<()> {
            let _result = dry_run_with_inspector::<L1ChainSpec, _, _, _, _>(
                blockchain,
                state,
                evm_config.to_cfg_env(hardfork),
                transaction,
                block_env,
                &HashMap::default(),
                &mut debugger,
            )?;

            Ok(())
        });

        let (stream, _) = server.accept()?;

        Ok(Self {
            debugger_handle,
            next_request_id: 1,
        })
    }

    /// Sends a request and waits for the response.
    ///
    /// # Panics
    ///
    /// Panics if the response's command or request sequence does not match the
    /// sent request; or if the response type is not `"response"`.
    pub fn send_request_and_wait_for_response(
        &mut self,
        command: impl ToString,
        arguments: serde_json::Value,
    ) -> edr_debugger_protocol::Response {
        let command = command.to_string();
        let seq = self.next_request_id;

        let request = edr_debugger_protocol::Request {
            arguments: Some(arguments),
            command: command.clone(),
            seq,
            type_: edr_debugger_protocol::RequestType::Request,
        };

        self.request_sender
            .send(request)
            .expect("Failed to send request");

        self.next_request_id += 1;

        let response = self
            .response_receiver
            .recv()
            .expect("Failed to receive response");

        assert_eq!(response.command, command);
        assert_eq!(response.request_seq, seq);
        assert_eq!(
            response.type_,
            edr_debugger_protocol::ResponseType::Response
        );

        response
    }

    /// Collects all available events.
    ///
    /// # Panics
    ///
    /// Panics if any of the received events does not have type `"event"`.
    pub fn collect_events(&self) -> Vec<edr_debugger_protocol::Event> {
        let mut events = Vec::new();

        loop {
            match self.event_receiver.try_recv() {
                Ok(event) => {
                    assert_eq!(event.type_, edr_debugger_protocol::EventType::Event);

                    events.push(event);
                }
                Err(TryRecvError::Disconnected) => unreachable!("Event channel disconnected"),
                Err(TryRecvError::Empty) => break,
            }
        }

        events
    }
}
