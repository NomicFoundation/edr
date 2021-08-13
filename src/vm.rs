// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: Copyright 2021 contributors to the Rethnet project.

use std::{
  error::Error,
  fmt::{Display, Formatter, Result as FmtResult},
};

use bytes::Bytes;
use evmodin::{tracing::Tracer, Host};

/// The result/error type emitted by the virtual machine.
pub type Result<T> = std::result::Result<T, VMError>;

/// An interpreter and executor of EVM bytecode.
///
/// The virtual machine is just a simple stack machine that executes ethereum
/// bytecode. It works as a state transformation function Y(S, T) = S'
/// Where (S) is the state of the world before running a transaction (T),
/// and (S') is the new wold state that is the result of executing the
/// transaction (T).
///
/// The virtual machine does not perform any I/O on its own and is also not
/// responsible for maintaining the state of the world, instead, read operations
/// of existing world state are delegated to the state storage subsystem, and
/// all changes to the state are returned as instance of a ['StateDiff'] struct
/// that describes what changes to the world state (S' - S) result from running
/// a transaction.
///
/// Note that the virtual machine is only invoked when we have a contract call
/// or contract creation transactions. Otherwise, for simple value transfers
/// the virtual machine is not involved and the value transfer happens at
// higher levels of abstraction.
pub struct VirtualMachine;

impl VirtualMachine {
  pub fn new() -> Result<VirtualMachine> {
    Ok(VirtualMachine)
  }
}

struct VirtualMachineHost;
impl Host for VirtualMachineHost {
  fn account_exists(&self, address: ethereum_types::Address) -> bool {
    todo!()
  }

  fn get_storage(
    &self,
    address: ethereum_types::Address,
    key: ethereum_types::H256,
  ) -> ethereum_types::H256 {
    todo!()
  }

  fn set_storage(
    &mut self,
    address: ethereum_types::Address,
    key: ethereum_types::H256,
    value: ethereum_types::H256,
  ) -> evmodin::host::StorageStatus {
    println!(
      "setting storage value at address: {}, key: {}, to: {}",
      address, key, value
    );
    evmodin::host::StorageStatus::Added
  }

  fn get_balance(&self, address: ethereum_types::Address) -> ethereum_types::U256 {
    todo!()
  }

  fn get_code_size(&self, address: ethereum_types::Address) -> ethereum_types::U256 {
    todo!()
  }

  fn get_code_hash(&self, address: ethereum_types::Address) -> ethereum_types::H256 {
    todo!()
  }

  fn copy_code(&self, address: ethereum_types::Address, offset: usize, buffer: &mut [u8]) -> usize {
    todo!()
  }

  fn selfdestruct(
    &mut self,
    address: ethereum_types::Address,
    beneficiary: ethereum_types::Address,
  ) {
    todo!()
  }

  fn call(&mut self, msg: &evmodin::Message) -> evmodin::Output {
    todo!()
  }

  fn get_tx_context(&self) -> evmodin::host::TxContext {
    todo!()
  }

  fn get_block_hash(&self, block_number: u64) -> ethereum_types::H256 {
    todo!()
  }

  fn emit_log(
    &mut self,
    address: ethereum_types::Address,
    data: &[u8],
    topics: &[ethereum_types::H256],
  ) {
    todo!()
  }

  fn access_account(&mut self, address: ethereum_types::Address) -> evmodin::host::AccessStatus {
    todo!()
  }

  fn access_storage(
    &mut self,
    address: ethereum_types::Address,
    key: ethereum_types::H256,
  ) -> evmodin::host::AccessStatus {
    println!("accessing storage address {}, key: {}", &address, &key);
    evmodin::host::AccessStatus::Cold
  }
}

struct VirtualMachineTracer;
impl Tracer for VirtualMachineTracer {
  const DUMMY: bool = false;

  fn notify_execution_start(
    &mut self,
    revision: evmodin::Revision,
    message: evmodin::Message,
    code: Bytes,
  ) {
    println!("execution started: {:#?}", &message);
    println!("execution started: {:#?}", &code);
    println!("execution revision: {:?}", &revision);
  }

  fn notify_instruction_start(
    &mut self,
    pc: usize,
    opcode: evmodin::OpCode,
    state: &evmodin::ExecutionState,
  ) {
    println!(
      "instruction start -> pc: {}, opcode: {}, state: {:#?}",
      &pc, opcode, &state
    );
  }

  fn notify_execution_end(&mut self, output: &evmodin::Output) {
    println!("execution end: {:#?}", &output);
  }
}

/// Describe errors that occur in the virtual machine.
///
/// Those are very low-level errors and are usually signalling things
/// like corrupt bytecode, invalid instructions, offsets, etc.
#[derive(Debug)]
pub struct VMError;

impl Display for VMError {
  fn fmt(&self, _: &mut Formatter<'_>) -> FmtResult {
    todo!()
  }
}

impl Error for VMError {}

#[cfg(test)]
mod tests {
  use anyhow::Result;
  use ethereum_types::{Address, U256};
  use evmodin::{AnalyzedCode, CallKind, Message, Revision};
  use hex_literal::hex;

  use super::*;

  /// compiles the standard "Storage" contract from remix.ethereum.org samples.
  /// The hex representation of the contract creation code is:
  /// ```
  /// 608060405234801561001057600080fd5b50610150806100206000396000f3fe6080604052
  /// 34801561001057600080fd5b50600436106100365760003560e01c80632e64cec11461003b
  /// 5780636057361d14610059575b600080fd5b610043610075565b60405161005091906100d9
  /// 565b60405180910390f35b610073600480360381019061006e919061009d565b61007e565b
  /// 005b60008054905090565b8060008190555050565b60008135905061009781610103565b92
  /// 915050565b6000602082840312156100b3576100b26100fe565b5b60006100c18482850161
  /// 0088565b91505092915050565b6100d3816100f4565b82525050565b600060208201905061
  /// 00ee60008301846100ca565b92915050565b6000819050919050565b600080fd5b61010c81
  /// 6100f4565b811461011757600080fd5b5056fea2646970667358221220404e37f487a89a93
  /// 2dca5e77faaf6ca2de3b991f93d230604b1b8daaef64766264736f6c63430008070033
  /// ```
  /// which corresponds to the following opcodes:
  /// ```
  /// .code
  ///   PUSH 80			contract Storage {\n\n    uint...
  ///   PUSH 40			contract Storage {\n\n    uint...
  ///   MSTORE 			contract Storage {\n\n    uint...
  ///   CALLVALUE 			contract Storage {\n\n    uint...
  ///   DUP1 			contract Storage {\n\n    uint...
  ///   ISZERO 			contract Storage {\n\n    uint...
  ///   PUSH [tag] 1			contract Storage {\n\n    uint...
  ///   JUMPI 			contract Storage {\n\n    uint...
  ///   PUSH 0			contract Storage {\n\n    uint...
  ///   DUP1 			contract Storage {\n\n    uint...
  ///   REVERT 			contract Storage {\n\n    uint...
  /// tag 1			contract Storage {\n\n    uint...
  ///   JUMPDEST 			contract Storage {\n\n    uint...
  ///   POP 			contract Storage {\n\n    uint...
  ///   PUSH #[$] 0000000000000000000000000000000000000000000000000000000000000000			contract Storage {\n\n    uint...
  ///   DUP1 			contract Storage {\n\n    uint...
  ///   PUSH [$] 0000000000000000000000000000000000000000000000000000000000000000			contract Storage {\n\n    uint...
  ///   PUSH 0			contract Storage {\n\n    uint...
  ///   CODECOPY 			contract Storage {\n\n    uint...
  ///   PUSH 0			contract Storage {\n\n    uint...
  ///   RETURN 			contract Storage {\n\n    uint...
  /// .data
  ///   0:
  ///     .code
  ///       PUSH 80			contract Storage {\n\n    uint...
  ///       PUSH 40			contract Storage {\n\n    uint...
  ///       MSTORE 			contract Storage {\n\n    uint...
  ///       CALLVALUE 			contract Storage {\n\n    uint...
  ///       DUP1 			contract Storage {\n\n    uint...
  ///       ISZERO 			contract Storage {\n\n    uint...
  ///       PUSH [tag] 1			contract Storage {\n\n    uint...
  ///       JUMPI 			contract Storage {\n\n    uint...
  ///       PUSH 0			contract Storage {\n\n    uint...
  ///       DUP1 			contract Storage {\n\n    uint...
  ///       REVERT 			contract Storage {\n\n    uint...
  ///     tag 1			contract Storage {\n\n    uint...
  ///       JUMPDEST 			contract Storage {\n\n    uint...
  ///       POP 			contract Storage {\n\n    uint...
  ///       PUSH 4			contract Storage {\n\n    uint...
  ///       CALLDATASIZE 			contract Storage {\n\n    uint...
  ///       LT 			contract Storage {\n\n    uint...
  ///       PUSH [tag] 2			contract Storage {\n\n    uint...
  ///       JUMPI 			contract Storage {\n\n    uint...
  ///       PUSH 0			contract Storage {\n\n    uint...
  ///       CALLDATALOAD 			contract Storage {\n\n    uint...
  ///       PUSH E0			contract Storage {\n\n    uint...
  ///       SHR 			contract Storage {\n\n    uint...
  ///       DUP1 			contract Storage {\n\n    uint...
  ///       PUSH 2E64CEC1			contract Storage {\n\n    uint...
  ///       EQ 			contract Storage {\n\n    uint...
  ///       PUSH [tag] 3			contract Storage {\n\n    uint...
  ///       JUMPI 			contract Storage {\n\n    uint...
  ///       DUP1 			contract Storage {\n\n    uint...
  ///       PUSH 6057361D			contract Storage {\n\n    uint...
  ///       EQ 			contract Storage {\n\n    uint...
  ///       PUSH [tag] 4			contract Storage {\n\n    uint...
  ///       JUMPI 			contract Storage {\n\n    uint...
  ///     tag 2			contract Storage {\n\n    uint...
  ///       JUMPDEST 			contract Storage {\n\n    uint...
  ///       PUSH 0			contract Storage {\n\n    uint...
  ///       DUP1 			contract Storage {\n\n    uint...
  ///       REVERT 			contract Storage {\n\n    uint...
  ///     tag 3			function retrieve() public vie...
  ///       JUMPDEST 			function retrieve() public vie...
  ///       PUSH [tag] 5			function retrieve() public vie...
  ///       PUSH [tag] 6			function retrieve() public vie...
  ///       JUMP [in]			function retrieve() public vie...
  ///     tag 5			function retrieve() public vie...
  ///       JUMPDEST 			function retrieve() public vie...
  ///       PUSH 40			function retrieve() public vie...
  ///       MLOAD 			function retrieve() public vie...
  ///       PUSH [tag] 7			function retrieve() public vie...
  ///       SWAP2 			function retrieve() public vie...
  ///       SWAP1 			function retrieve() public vie...
  ///       PUSH [tag] 8			function retrieve() public vie...
  ///       JUMP [in]			function retrieve() public vie...
  ///     tag 7			function retrieve() public vie...
  ///       JUMPDEST 			function retrieve() public vie...
  ///       PUSH 40			function retrieve() public vie...
  ///       MLOAD 			function retrieve() public vie...
  ///       DUP1 			function retrieve() public vie...
  ///       SWAP2 			function retrieve() public vie...
  ///       SUB 			function retrieve() public vie...
  ///       SWAP1 			function retrieve() public vie...
  ///       RETURN 			function retrieve() public vie...
  ///     tag 4			function store(uint256 num) pu...
  ///       JUMPDEST 			function store(uint256 num) pu...
  ///       PUSH [tag] 9			function store(uint256 num) pu...
  ///       PUSH 4			function store(uint256 num) pu...
  ///       DUP1 			function store(uint256 num) pu...
  ///       CALLDATASIZE 			function store(uint256 num) pu...
  ///       SUB 			function store(uint256 num) pu...
  ///       DUP2 			function store(uint256 num) pu...
  ///       ADD 			function store(uint256 num) pu...
  ///       SWAP1 			function store(uint256 num) pu...
  ///       PUSH [tag] 10			function store(uint256 num) pu...
  ///       SWAP2 			function store(uint256 num) pu...
  ///       SWAP1 			function store(uint256 num) pu...
  ///       PUSH [tag] 11			function store(uint256 num) pu...
  ///       JUMP [in]			function store(uint256 num) pu...
  ///     tag 10			function store(uint256 num) pu...
  ///       JUMPDEST 			function store(uint256 num) pu...
  ///       PUSH [tag] 12			function store(uint256 num) pu...
  ///       JUMP [in]			function store(uint256 num) pu...
  ///     tag 9			function store(uint256 num) pu...
  ///       JUMPDEST 			function store(uint256 num) pu...
  ///       STOP 			function store(uint256 num) pu...
  ///     tag 6			function retrieve() public vie...
  ///       JUMPDEST 			function retrieve() public vie...
  ///       PUSH 0			uint256
  ///       DUP1 			number
  ///       SLOAD 			number
  ///       SWAP1 			return number
  ///       POP 			return number
  ///       SWAP1 			function retrieve() public vie...
  ///       JUMP [out]			function retrieve() public vie...
  ///     tag 12			function store(uint256 num) pu...
  ///       JUMPDEST 			function store(uint256 num) pu...
  ///       DUP1 			num
  ///       PUSH 0			number
  ///       DUP2 			number = num
  ///       SWAP1 			number = num
  ///       SSTORE 			number = num
  ///       POP 			number = num
  ///       POP 			function store(uint256 num) pu...
  ///       JUMP [out]			function store(uint256 num) pu...
  ///     tag 16			-License-Identifier: GPL-3.0\n...
  ///       JUMPDEST 			-License-Identifier: GPL-3.0\n...
  ///       PUSH 0			>=0.7
  ///       DUP2 			\n * @d
  ///       CALLDATALOAD 			title Storage\n * @de
  ///       SWAP1 			\n/**\n * @title Storage\n * @...
  ///       POP 			\n/**\n * @title Storage\n * @...
  ///       PUSH [tag] 18			 retrieve value in a variable\...
  ///       DUP2 			le\n *
  ///       PUSH [tag] 19			 retrieve value in a variable\...
  ///       JUMP [in]			 retrieve value in a variable\...
  ///     tag 18			 retrieve value in a variable\...
  ///       JUMPDEST 			 retrieve value in a variable\...
  ///       SWAP3 			-License-Identifier: GPL-3.0\n...
  ///       SWAP2 			-License-Identifier: GPL-3.0\n...
  ///       POP 			-License-Identifier: GPL-3.0\n...
  ///       POP 			-License-Identifier: GPL-3.0\n...
  ///       JUMP [out]			-License-Identifier: GPL-3.0\n...
  ///     tag 11			orage {\n\n    uint256 number;...
  ///       JUMPDEST 			orage {\n\n    uint256 number;...
  ///       PUSH 0			ue in
  ///       PUSH 20			  
  ///       DUP3 			e to stor
  ///       DUP5 			 num va
  ///       SUB 			aram num value to store
  ///       SLT 			* @param num value to store\n ...
  ///       ISZERO 			   * @param num value to store...
  ///       PUSH [tag] 21			   * @param num value to store...
  ///       JUMPI 			   * @param num value to store...
  ///       PUSH [tag] 22			\n    function store(uint256 n...
  ///       PUSH [tag] 23			\n    function store(uint256 n...
  ///       JUMP [in]			\n    function store(uint256 n...
  ///     tag 22			\n    function store(uint256 n...
  ///       JUMPDEST 			\n    function store(uint256 n...
  ///     tag 21			   * @param num value to store...
  ///       JUMPDEST 			   * @param num value to store...
  ///       PUSH 0			v
  ///       PUSH [tag] 24			\n    function retrieve() publ...
  ///       DUP5 			(uint25
  ///       DUP3 			 retur
  ///       DUP6 			public vi
  ///       ADD 			e() public view return
  ///       PUSH [tag] 16			\n    function retrieve() publ...
  ///       JUMP [in]			\n    function retrieve() publ...
  ///     tag 24			\n    function retrieve() publ...
  ///       JUMPDEST 			\n    function retrieve() publ...
  ///       SWAP2 			r'\n     */\n    function retr...
  ///       POP 			r'\n     */\n    function retr...
  ///       POP 			Return value \n     * @return ...
  ///       SWAP3 			orage {\n\n    uint256 number;...
  ///       SWAP2 			orage {\n\n    uint256 number;...
  ///       POP 			orage {\n\n    uint256 number;...
  ///       POP 			orage {\n\n    uint256 number;...
  ///       JUMP [out]			orage {\n\n    uint256 number;...
  ///     tag 25			r;\n    }\n}
  ///       JUMPDEST 			r;\n    }\n}
  ///       PUSH [tag] 27
  ///       DUP2
  ///       PUSH [tag] 28
  ///       JUMP [in]
  ///     tag 27
  ///       JUMPDEST
  ///       DUP3
  ///       MSTORE
  ///       POP 			r;\n    }\n}
  ///       POP 			r;\n    }\n}
  ///       JUMP [out]			r;\n    }\n}
  ///     tag 8
  ///       JUMPDEST
  ///       PUSH 0
  ///       PUSH 20
  ///       DUP3
  ///       ADD
  ///       SWAP1
  ///       POP
  ///       PUSH [tag] 30
  ///       PUSH 0
  ///       DUP4
  ///       ADD
  ///       DUP5
  ///       PUSH [tag] 25
  ///       JUMP [in]
  ///     tag 30
  ///       JUMPDEST
  ///       SWAP3
  ///       SWAP2
  ///       POP
  ///       POP
  ///       JUMP [out]
  ///     tag 28
  ///       JUMPDEST
  ///       PUSH 0
  ///       DUP2
  ///       SWAP1
  ///       POP
  ///       SWAP2
  ///       SWAP1
  ///       POP
  ///       JUMP [out]
  ///     tag 23
  ///       JUMPDEST
  ///       PUSH 0
  ///       DUP1
  ///       REVERT
  ///     tag 19
  ///       JUMPDEST
  ///       PUSH [tag] 38
  ///       DUP2
  ///       PUSH [tag] 28
  ///       JUMP [in]
  ///     tag 38
  ///       JUMPDEST
  ///       DUP2
  ///       EQ
  ///       PUSH [tag] 39
  ///       JUMPI
  ///       PUSH 0
  ///       DUP1
  ///       REVERT
  ///     tag 39
  ///       JUMPDEST
  ///       POP
  ///       JUMP [out]
  ///     .data
  /// ```
  #[test]
  fn evmodin_smoke() -> Result<()> {
    let bytecode = hex!(
      "608060405234801561001057600080fd5b50610150806100206000396000"
      "f3fe608060405234801561001057600080fd5b5060043610610036576000"
      "3560e01c80632e64cec11461003b5780636057361d14610059575b600080"
      "fd5b610043610075565b60405161005091906100d9565b60405180910390"
      "f35b610073600480360381019061006e919061009d565b61007e565b005b"
      "60008054905090565b8060008190555050565b6000813590506100978161"
      "0103565b92915050565b6000602082840312156100b3576100b26100fe56"
      "5b5b60006100c184828501610088565b91505092915050565b6100d38161"
      "00f4565b82525050565b60006020820190506100ee60008301846100ca56"
      "5b92915050565b6000819050919050565b600080fd5b61010c816100f456"
      "5b811461011757600080fd5b5056fea2646970667358221220404e37f487"
      "a89a932dca5e77faaf6ca2de3b991f93d230604b1b8daaef64766264736f"
      "6c63430008070033");

    let message = Message {
      kind: CallKind::Create,
      is_static: true,
      depth: 0,
      gas: 200,
      destination: Address::zero(),
      sender: Address::from_low_u64_be(1),
      input_data: vec![].into(),
      value: U256::zero(),
    };

    let output = AnalyzedCode::analyze(bytecode).execute(
      &mut VirtualMachineHost,
      &mut VirtualMachineTracer,
      None,
      message,
      Revision::latest(),
    );

    // call store(12)
    let contract_call = Message {
      kind: CallKind::CallCode,
      is_static: false,
      depth: 0,
      gas: 200000,
      destination: Address::zero(),
      sender: Address::zero(),
      input_data: hex!("6057361d000000000000000000000000000000000000000000000000000000000000000c") // [ "uint256 num", "12" ]
        .to_vec()
        .into(),
      value: U256::zero(),
    };

    let contract_call_output = AnalyzedCode::analyze(output.output_data.to_vec()).execute(
      &mut VirtualMachineHost,
      &mut VirtualMachineTracer,
      None,
      contract_call,
      Revision::latest(),
    );

    println!("contract creation output: {:#?}", &output);
    println!("contract call output: {:#?}", &contract_call_output);

    assert!(true);
    Ok(())
  }
}
