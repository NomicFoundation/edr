# edr_spike_evm2

Spike checking whether EDR's L1 execution can run on [evm2](https://github.com/alloy-rs/evm2)
(rev `35aa328`) instead of revm. It runs the same transactions on revm 43 (through EDR's
`L1ChainSpec`) and on evm2, both over EDR's own `PersistentStateTrie`, and compares the results.

```sh
cargo run --release -p edr_spike_evm2            # parity and state commit
cargo run --release -p edr_spike_evm2 -- --timing # plus timings
```

## Validated

**State as an evm2 database.** A small adapter (`EdrStateDatabase`) lets evm2 read
`PersistentStateTrie` without converting the state.

**Execution parity, 48 cases (Prague; precompiles also on Osaka).** Identical stop reason,
output, created address, gas used, logs and state changes, plus identical opcode steps (pc,
opcode, full stack). The cases cover:

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
| Output of a create rejected for `0xEF` code | `0xef` | empty |
| Selfdestruct to self of an existing contract | no selfdestruct recorded | selfdestruct recorded |

All four reach Hardhat through `edr_napi` (`trace.rs`, `result.rs`). Not yet checked against
Geth.

**State commit.** evm2's `account_changes()`, converted into the `EvmState` that EDR's `commit`
takes, gives the same state root as revm in all 48 cases. The same holds over a chained run of
all 48 transactions on each engine's own state. Use `account_changes()`, not
`StateChangeSink`: the sink doesn't carry the `touched` flag that EDR's commit reads.

**Performance (one machine, indicative).** evm2/revm time ratio: 0.55 without an inspector,
1.10 recording pc and opcode per step, 1.07 with the full stack. Each transaction loads its
contract code once, and building evm2's bytecode takes about 1 µs.

**Build.** evm2 compiles with default features off (no native dependencies), on the same alloy
majors as EDR and revm.

## Not covered

Amsterdam, OP, EIP-7702 and typed or blob transactions, block-level execution (system calls,
withdrawals, merging several transactions per block), fork state, validation errors and
hardfork mapping, and `debug_trace*` Geth output.
