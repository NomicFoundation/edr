# edr_spike_evm2

Spike checking whether EDR's L1 execution can run on [evm2](https://github.com/alloy-rs/evm2)
(rev `35aa328`) instead of revm. It runs the same transactions on revm 43 (through EDR's
`L1ChainSpec`) and on evm2, both over EDR's own `PersistentStateTrie`, and compares the results.

```sh
cargo run --release -p edr_spike_evm2                  # all sections
cargo run --release -p edr_spike_evm2 -- --round-trip  # adapter round trip only
cargo run --release -p edr_spike_evm2 -- --timing      # plus timings
```

## Validated

**State as an evm2 database.** A small adapter (`EdrStateDatabase`) lets evm2 read
`PersistentStateTrie` without converting the state.

**Execution parity, 48 cases (Prague; precompiles also on Osaka).** Identical output, created
address, gas used, logs and state changes, plus identical opcode steps (pc, opcode, full stack).
The stop reason matches too, except the `SuccessReason` of a successful create (see call
traces). The cases cover:

- transfers, a 170k-step loop, reverts with data;
- halts: `INVALID`, undefined opcode, out of gas, stack underflow, invalid jump;
- CREATE transactions (success, init reverts, init halts, `0xEF` code, selfdestruct in init);
- nested calls: revert, static-call write, delegatecall, precompile call, CREATE/CREATE2, value
  transfer, call to a codeless account;
- recursion down to out-of-gas halts (386 frames);
- every Prague precompile with valid and invalid input; P256VERIFY inactive on Prague and active
  on Osaka;
- state edge cases: EIP-161 touches of empty and missing accounts, the RIPEMD-160 out-of-gas
  rule, storage clear/restore/write, selfdestruct of an existing contract and to self, coinbase
  fees.

**Call traces.** evm2-inspectors' `CallTraceArena` matches revm-inspectors' field by field
(tree, caller/address, kind, value, input, gas limit, refund counter, selfdestruct, logs and
their positions), using the config of EDR's `SolidityTracingInspector`, except in four cases:

| Case | revm-inspectors | evm2-inspectors |
|---|---|---|
| `gas_used` of a halted frame | gas spent before the halt | all gas burned |
| Status of a successful create (and the tx's `SuccessReason`) | always `Return` | how the init code stopped (`Stop` / `SelfDestruct`) |
| Output of a create rejected at code deposit (`0xEF` code, code size limit) | the returned code | empty |
| Selfdestruct to self of an existing contract | no selfdestruct recorded | selfdestruct recorded |

All four reach Hardhat through `edr_napi` (`trace.rs`, `result.rs`). Not yet checked against
Geth.

**State commit.** evm2's `account_changes()`, converted into the `EvmState` that EDR's `commit`
takes, gives the same state root as revm in all 48 cases. The same holds over a chained run of
all 48 transactions on each engine's own state. Use `account_changes()`, not
`StateChangeSink`: the sink doesn't carry the `touched` flag that EDR's commit reads.

**Adapter round trip, 177 cases.** `adapter::dry_run` has the same inputs and outputs as
`edr_evm::dry_run::<L1ChainSpec, ..>`: `CfgEnv`, EDR's block env (`HeaderAndEvmSpec`), an
`L1SignedTransaction` and revm `PrecompileFn`s in; `ExecutionResult` + `EvmState`, or a
`TransactionError`, out. Every case runs through both, and results (full `ExecutionResult`,
including `ResultGas`), errors, precompile addresses and committed state roots are equal in all
177. The cases cover:

- the 48 parity cases;
- every block env field (number, timestamp, coinbase, base fee, prevrandao, gas limit, blob base
  fee, chain id, gas price, origin, `BLOBHASH`, `BLOCKHASH` through EDR's blockchain);
- all L1 transaction types: pre-EIP-155 and EIP-155 legacy, EIP-2930 with an access list,
  EIP-1559 (call and create), EIP-4844 with blobs, EIP-7702 with valid, any-chain and
  wrong-nonce authorizations;
- every `CfgEnv` setting EDR uses: chain id, EIP-3607 on/off, `set_guarantees` (balance, nonce,
  block gas limit), code and initcode size limits, the EIP-7825 cap (set and Osaka default),
  block gas limit on/off;
- validation errors: nonce too low/high, intrinsic gas, EIP-7623 floor, fee cap below base fee,
  priority fee above max fee, insufficient funds, empty/too many/wrong-version blobs, blob fee
  cap, empty authorization list, each typed transaction before its hardfork;
- EDR's custom precompiles with revm's `PrecompileFn` signature, run unchanged on evm2 through a
  `PrecompileProvider` wrapper: P256VERIFY at `0x100` and overriding `0x04`, a reverting and a
  halting precompile, from the transaction and from a contract;
- every EDR L1 hardfork from Byzantium to Osaka.

The adapter needed four rules to match revm:

- a successful create transaction reports `SuccessReason::Return`;
- the halt message of a top-level precompile call, which evm2 drops, is captured in the
  precompile wrapper and reported as `PrecompileErrorWithContext` (`edr_napi_core` serializes it);
- an unset `max_blobs_per_tx` means no per-transaction limit, as in revm;
- in traces, the create status and the selfdestruct-to-self differences above are aligned with
  revm-inspectors.

The other two trace differences (halted frame `gas_used`, rejected create output) can't be
derived from evm2's arena: 34 of the 177 traces differ only in those.

Hardfork mapping is one-to-one: EDR maps every L1 hardfork to a revm `SpecId`, and revm 43's 15
`SpecId`s exist in evm2 with the same names.

**Performance (one machine, indicative).** evm2/revm time ratio: 0.55 without an inspector,
1.10 recording pc and opcode per step, 1.07 with the full stack. Each transaction loads its
contract code once, and building evm2's bytecode takes about 1 µs.

**Build.** evm2 compiles with default features off (no native dependencies), on the same alloy
majors as EDR and revm.

## Not covered

Amsterdam, OP, block-level execution (system calls, withdrawals, several transactions per block),
fork state, database errors (the adapter turns them into `TransactionError::Custom`), header
validation errors (`PrevrandaoNotSet`, `ExcessBlobGasNotSet`), revm validation errors with no evm2
counterpart (e.g. `OverflowPaymentInTransaction`, `AuthorizationListInvalidFields`), and
`debug_trace*` Geth output.
