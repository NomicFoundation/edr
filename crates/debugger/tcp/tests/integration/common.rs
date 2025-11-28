use std::{
    collections::BTreeMap,
    io::{Read, Write},
    net::{Ipv6Addr, SocketAddr, TcpListener, TcpStream},
    str::FromStr as _,
};

use edr_block_api::{GenesisBlockFactory as _, GenesisBlockOptions};
use edr_block_header::BlockConfig;
use edr_blockchain_api::{BlockchainMetadata as _, StateAtBlock as _};
use edr_chain_l1::{
    L1ChainSpec, L1SignedTransaction, L1_BASE_FEE_PARAMS, L1_MIN_ETHASH_DIFFICULTY,
};
use edr_debugger_tcp::create_tcp_debugger;
use edr_evm::dry_run_with_inspector;
use edr_evm_spec::{config::EvmConfig, BlockEnv};
use edr_primitives::{Address, Bytes, HashMap, TxKind, B256, U256};
use edr_provider::spec::LocalBlockchainForChainSpec;
use edr_signer::{public_key_to_address, SecretKey};
use edr_state_api::{AccountModifierFn, State, StateDiff, StateError};
use edr_test_blockchain::deploy_contract;
use edr_test_utils::secret_key::secret_key_from_str;
use serde::Serialize;

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

pub struct ResponseAndEvents {
    pub response: edr_debugger_protocol::Response,
    pub events: Vec<edr_debugger_protocol::Event>,
}
pub struct TcpDebuggerFixture {
    debugger_handle: std::thread::JoinHandle<anyhow::Result<()>>,
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

        let secret_key = secret_key_from_str(edr_defaults::SECRET_KEYS[0])?;
        let caller = public_key_to_address(secret_key.public_key());

        let mut state = blockchain.state_at_block_number(0, &BTreeMap::new())?;
        state.modify_account(
            caller,
            AccountModifierFn::new(Box::new(|balance, _nonce, _code| {
                *balance = U256::from(100_000_000_000_000u128);
            })),
        )?;

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

        let hardfork = blockchain.hardfork();

        let debugger_handle = std::thread::spawn(move || -> anyhow::Result<()> {
            let mut debugger =
                create_tcp_debugger(server_address, true).expect("Failed to connect to server");

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

        let (tcp_stream, _) = server.accept()?;

        Ok(Self {
            debugger_handle,
            next_request_id: 1,
            tcp_stream,
        })
    }

    /// Sends a request and waits for the resulting protocol messages (responses
    /// and events).
    pub fn send_request_and_wait_for_protocol_messages(
        &mut self,
        command: impl ToString,
        arguments: impl Serialize,
    ) -> ResponseAndEvents {
        let command = command.to_string();
        let seq = self.next_request_id;

        let arguments = serde_json::to_value(arguments).expect("Argument should serialize");

        let request = edr_debugger_protocol::Request {
            arguments: Some(arguments),
            command: command.clone(),
            seq,
            type_: edr_debugger_protocol::RequestType::Request,
        };

        let request = serde_json::to_string(&request).expect("Failed to serialize request");
        self.tcp_stream
            .write_all(request.as_bytes())
            .expect("Failed to write request");

        self.next_request_id += 1;

        let mut buffer = String::new();

        let mut events = Vec::new();
        let mut found_response = None;
        loop {
            println!("Waiting for protocol message...");
            match self.tcp_stream.read_to_string(&mut buffer) {
                Ok(_) => (),
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(error) => panic!("Failed to read from TCP stream: {error}"),
            }

            let message = serde_json::from_str::<edr_debugger_protocol::ProtocolMessage>(&buffer)
                .expect("Failed to deserialize protocol message");

            println!("Received protocol message: {:?}", message);

            if message.type_ == "event" {
                let event: edr_debugger_protocol::Event =
                    serde_json::from_str(&buffer).expect("Failed to deserialize event");

                events.push(event);
            } else if message.type_ == "response" {
                let response: edr_debugger_protocol::Response = serde_json::from_str(&buffer)
                    .expect("Failed to deserialize response: {buffer}");

                assert_eq!(response.command, command);
                assert_eq!(response.request_seq, seq);
                assert_eq!(
                    response.type_,
                    edr_debugger_protocol::ResponseType::Response
                );

                found_response = Some(response);

                // We except exactly one response per request and zero or more events, so
                // reading should become non-blocking now.
                self.tcp_stream
                    .set_nonblocking(true)
                    .expect("Failed to set non-blocking");
            } else {
                panic!("Unexpected protocol message type: {}", message.type_);
            }
        }

        // Restore blocking mode for future requests.
        self.tcp_stream
            .set_nonblocking(false)
            .expect("Failed to set non-blocking");

        ResponseAndEvents {
            response: found_response.expect("Failed to get response"),
            events,
        }
    }

    /// Collects all available events.
    ///
    /// # Panics
    ///
    /// Panics if any of the received events does not have type `"event"`.
    pub fn collect_events(&mut self) -> Vec<edr_debugger_protocol::Event> {
        let mut events = Vec::new();

        let mut buffer = String::new();
        loop {
            match self.tcp_stream.read_to_string(&mut buffer) {
                Ok(_) => (),
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(error) => panic!("Failed to read protocol message: {error}"),
            }

            let message = serde_json::from_str::<edr_debugger_protocol::ProtocolMessage>(&buffer)
                .expect("Failed to deserialize protocol message");

            assert!(message.type_ == "event");
            let event: edr_debugger_protocol::Event =
                serde_json::from_str(&buffer).expect("Failed to deserialize event");

            events.push(event);

            if events.len() == 1 {
                // We expect one or more events. Since we've read one event, we can set the
                // stream to non-blocking mode now.
                self.tcp_stream
                    .set_nonblocking(true)
                    .expect("Failed to set non-blocking");
            }
        }

        // Restore blocking mode for future requests.
        self.tcp_stream
            .set_nonblocking(false)
            .expect("Failed to set non-blocking");

        events
    }

    pub fn wait_for_termination(mut self) -> anyhow::Result<()> {
        self.debugger_handle
            .join()
            .expect("Debugger thread panicked")
    }
}
