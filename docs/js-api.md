# Rethnet JavaScript API

This document describes the design and rationale behind the exposed JS API of rethnet.

## Milestone 1 - August 2021

This milestone attempts to deliver a drop-in replacement for `EthereumJS/vm` using `rethnet`. The publicly available API is:

```ts

/**
 * Represents an unsigned 256 bit integer.
 * This is the basic unit of storage in EVM.
 */
class U256 {
  /**
   * Converts a JS numberical value to an instance of U256.
   * This is a more explicit version of the constructor.
   */
  static fromNumber(value: number): U256;

  /**
   * Converts a JS hex-encoded numeric value to an instance of U256.
   * This is a more explicit version of the constructor.
   */
  static fromHex(value: string): U256;

  /**
   * Represents a 0-valued 256-bit unsigned integer.
   */
  static zero(): U256;

  /**
   * Constructs a new unsigned 256 bit integer.
   */
  constructor(value: number | string);

  /**
   * Prints a string representation of the u256 value.
   */
  public toString(): string;
}

/**
 * Represents an unsigned 160 bit integer used to address accounts on the chain.
 */
class Address {
  /**
   * Converts a JS numberical value to an instance of U256.
   * This is a more explicit version of the constructor.
   */
  static fromNumber(value: number): Address;

  /**
   * Converts a JS hex-encoded numeric value to an instance of U256.
   * This is a more explicit version of the constructor.
   */
  static fromHex(value: string): Address;

  /**
   * Represents the zero address.
   * This is where base fees are sent since London fork and EIP1559.
   */
  static zero(): Address;

  /**
   * Generates a new account address along with its 256-bit secret key.
   */
  static random(): [Address, U256];

  /**
   * Constructs a new unsigned 256 bit integer.
   */
  constructor(value: number | string);

  /**
   * Prints a string representation of the address.
   */
  public toString(): string;
}

/**
 * Represents a snapshot of the contract storage.
 * 
 * It is a mapping of 256bit int slots to 256 bit int values.
 * 
 * This type could represent either a state diff in a contract
 * or the entire contract state. All slots that do not have a
 * corresponding value are treated as if they had a zero value.
 * This means that deleting a value from the contract storage is
 * expressed through setting a storage slot's value to zero.
 */
type ContractState = Record<U256, U256>;

/**
 * Represents the state of an account in the blockchain world state
 */
class AccountState {
  balance: NonNullable<U256>;
  nonce: NonNullable<U256>;
  stateRoot: Readonly<NonNullable<U256>>; // zero if EOA, r/o by the API, changes only through tx execution
  codeHash: Readonly<U256>; // hash of an empty string for EOA, immutable for contract accounts.
}

/**
 * Represents the result of executing a transaction on the blockchain.
 * 
 * When transactions are executed, the resulting changes to the state, such
 * as account nonces, contract storage, etc, are not immediately applied, instead
 * a state diff is returned and the user of the API has to decide what to do with
 * the diff, either to apply it, validate it, or otherwise process it.
 */
class StateDiff {
  accounts: Record<Address, AccontState>;
  contracts: Record<Address, ContractState>;
}

/**
 * Implements an abstract interface for storage engines with read-only access.
 */
interface IReadOnlyStorageEngine {
  /**
   * Returns the current merklee proof of the entire blockchain state.
   * 
   * This value is relevant only when doing fullly synced blockchain, 
   * a pretty uncommon use case for this API. Most expected use cases
   * of the JS api are going to deal with partially or lazily synced state.
   */
  storageRootHash(): Promise<U256>;

  /**
   * Returns the entire stored state on the blockchain.
   * 
   * Be very careful when calling this method and use it only
   * on small and local test blockchains. Make sure you really
   * know what you are doing when calling this!
   */
  retrieveAll(): Promise<StateDiff>;

  /**
   * Returns the account state for a given address.
   * 
   * If there is no account under the given address, then a null value is returned.
   */
  retreiveAccount(address: Address): Promise<AccountState | null>;

  /**
   * Retreives the entire state of a contract address.
   * 
   * If called for an EOA address or a non-existant address, this method
   * will return a null value
   */
  retreiveContractStateSlot(address: Address): Promise<ContractState | null>;

  /**
   * Retreives a storage slot's value from contract's account address.
   * 
   * If the address points to an EOA or a non-existing address then this will
   * return null. If the address points a a valid contract address but the storage
   * slot is not used then an u256  zero value is retuned (Eth EVM semantics).
   */
  retreiveContractStateSlot(address: Address, slot: U256): Promise<U256 | null>;
}

/**
 * Implements an abstract interface for storage engines with write access.
 */
interface IMutableStorageEngine {
  /**
   * Persists a storage diff to blockchain's persistent state store.
   * 
   * This function is usually called when the user of the API wants to persist
   * the result of running a transaction and make later transactions work with
   * newer state.
   */
  apply(diff: StateDiff): Promise<void>;

  /**
   * Deletes an account from the blockchain.
   * 
   * If the account does not exist, it returns null.
   * If the account is a contract account with its own storage, that storage will be deleted as well
   * if the account is an EOA then it will be deleted. In cases when the address is pointing
   * to an existing account, the returned promise will hold the latest snapshot of stored account data.
   */
  deleteAccount(address: Address): Promise<AccountState | null>;
}

/**
 * Defines the abstract interface that needs to be implemented
 * by all storage engines that perform persistent I/O.
 */
interface IStorageEngine : IReadOnlyStorageEngine, IWriteOnlyStorageEngine {}

/**
 * InMemory storage used mostly for short-lived tests.
 * Data stored using this engine is lost once the instance 
 * of the Blockchain type that owns it dies.
 */
class InMemoryStorage : IStorageEngine { /* ... */ }

/**
 * Uses Browser's LocalStorage as its permanent storage for use cases
 * where this API is called on the client side.
 */
class LocalStorage : IStorageEngine { /* ...  */ }

/**
 * Uses a super-fast on-disk persistent storage.
 * for more details see: https://github.com/erthink/libmdbx
 */
class MdbxStorage : IStorageEngine { /* ... */ }

/**
 * EVM Version.
 * 
 * EVM versions can be specified for the entire blockchain or for 
 * a window of blocks.
 */
enum Hardfork {
  // The Frontier revision.
  // The one Ethereum launched with.
  Frontier = 0,

  // https://eips.ethereum.org/EIPS/eip-606
  Homestead = 1,

  // https://eips.ethereum.org/EIPS/eip-608
  Tangerine = 2,

  // https://eips.ethereum.org/EIPS/eip-607
  Spurious = 3,

  // https://eips.ethereum.org/EIPS/eip-609
  Byzantium = 4,

  // https://eips.ethereum.org/EIPS/eip-1013
  Constantinople = 5,

  /// https://eips.ethereum.org/EIPS/eip-1716
  Petersburg = 6,

  // https://eips.ethereum.org/EIPS/eip-1679
  Istanbul = 7,

  // https://github.com/ethereum/eth1.0-specs/blob/master/network-upgrades/mainnet-upgrades/berlin.md
  Berlin = 8,
  
  // https://github.com/ethereum/eth1.0-specs/blob/master/network-upgrades/mainnet-upgrades/london.md
  London = 9, 

  // The Shanghai revision. (upcoming)
  Shanghai = 10, 
}

// static functions on the Hardfork enum
namespace Hardfork {
  /**
   * Given a block number it returns the EVM revision that is/was used for that block on mainnet.
   * TODO: Add support for other chains (testnets) asaide from mainnet.
   */
  function revisionForBlockNumber(blockNumber: Number): Hardfork;
}

/**
 * Represents the result of running a transaction.
 * 
 * Which is the actual status code from the VM and all
 * resulting state diffs.
 */
type TransactionResult = [ExecutionStatus, StateDiff];

/**
 * Represents an instance of a Blockchain.
 * Design assumption: This is a scoped instance and many
 * independent instances of this type may coexist concurrently
 * within one process.
 * 
 * This type does not implement a virtual machine for executing
 * contracts or a consensus algorithm. It mereley glues together
 * other individual components into a functional API gateway to 
 * creating private blockchains.
 */
class Blockchain {
  /**
   * Creates a new blockchain instance backed by the specified storage engine.
   * 
   * An optional parameter hardforkMap specifies the block numbers for each of the EVM
   * upgrades. By default it will use the official mainnet transision table: https://ethereum.org/en/history/
   */
  constructor(storage: IStorageEngine, hardforkMap: Record<U256, Hardfork> | null = null);

  /**
   * Returns an instance of the underlying virtual machine.
   * 
   * Optionally if a block number is specified, then a virtual machine 
   * instantiated with the appropriate VM revision will be used, otherwise
   * the latest revision is returned. See: https://ethereum.org/en/history/
   */
  virtualMachine(blockNo: number | null = null): VirtualMachine;

  /**
   * Runs a transaction on the blockchain without persisting the resulting state diff.
   */
  simulateTransaction(tx: Transaction, blockNo: number | null = null): Promise<TransactionResult> {
    return virtualMachine(blockNo).executeTransaction(tx);
  }

  /**
   * Runs a transaction on the blockchain and persists the resulting state diff.
   */
  applyTransaction(tx: Transaction, blockNo: number | null = null): Promise<TransactionResult> {
    let stateDiff = await simulateTransaction(tx, blockNo);
    await this.storage.apply(stateDiff[1]);
    return stateDiff;
  }
}

/**
 * A status code for the result of a transaction execution.
 */
enum ExecutionStatus {
  Success,
  Failure,
  Revert,
  OutOfGas,
  InvalidInstruction,
  UndefinedInstruction,
  StackOverflow,
  StackUnderflow,
  BadJumpDestination,
  InvalidMemoryAccess,
  CallDepthExceeded,
  StaticModeViolation,
  PrecompileFailure,
  ArgumentOutOfRange,
  InsuffucuentBalance,
  InternalError // TODO: Should carry more info.
}

/**
 * Represents an instance of an Ethereum Virtual Machine.
 * Design assumptions: This is a scoped instance and many 
 * independent instances of this type may coexist concurrently
 * within one process.
 * 
 * This type does not perform any I/O and all persistent operation
 * are handled though a separate subsystem.
 */
class VirtualMachine {
  /**
   * Creates an instance of a virtual machine with a specified hardfork and I/O interface.
   * 
   * The usuall usage and ownership pattern of this type is that a blockchain will hold several instances
   * of a virtual machine, each tied to a specific block number range and a version that correspond to 
   * network updates over time.
   */
  constructor(revision: Hardfork, state: IReadOnlyStorageEngine);

  /**
   * Runs a transaction and returns its status code alond with any changes to the state
   * that may have occured.
   * 
   * All changes to account balances that are a result of gas usage, transfers, etc, 
   * are also going to be reflected in this state diff.
   *
   * The VM does not have write access to the storage, the blockchain instance MAY apply
   * the state diff to its storage if the user of the API wants to.
   */
  executeTransaction(tx: Transaction): Promise<TransactionResult>;
}

/**
 * Represents an Ethereum transaction. 
 * 
 * This is the basic and only unit of invocation in any EVM-based blockchain
 * 
 * TODO: Design this, accounting for EIP2930, EIP1559, and typed transactions.
 */
class Transaction {
  /**
   * Instantiate a transaction from the serialized tx.
   *
   * Format: `rlp([nonce, gasPrice, gasLimit, to, value, data, v, r, s])`
   */
  static fromRlp(buf: Buffer): Transaction;

  nonce: U256,
  gasPrice: U256,
  gasLimit: U256,
  to: Address,
  value: U256,
  data: Buffer

  v: U256
  r: U256
  s: U256
}

```