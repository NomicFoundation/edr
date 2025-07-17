# Predeploys

Pre-deployed smart contracts, or predeploys in short, exist on layer 2 (L2) blockchains like Optimism at pre-determined addresses in the genesis state.

To mimic this behaviour, EDR allows users to specify a custom genesis state containing - among others - predeploys. To make it easy to configure common genesis states, EDR's N-API package exposes functions for the layer 1 (L1) genesis state and OP genesis state, called `l1_genesis_state` and `op_genesis_state` respectively.

Whereas the bytecode of L1 predeploys has remained constant over time, OP predeploys have changed during hardforks. When updating bytecode, we follow this process:

1. Determine the address of the predeploy.
2. Look up the predeploy on Etherscan using its address.
3. Check whether the predeploy was upgraded, e.g. based on "events".
4. If any changes occurred, copy the deployed bytecode.
5. If any changes occurred, copy the state.

For example, for the `GasPriceOracle` predeploy of OP stack:

1. The address of the predeploy can be found in `crates/edr_op/src/predeploys.rs`: `0x420000000000000000000000000000000000000f`.
2. The predeploy on Etherscan: <https://optimistic.etherscan.io/address/0x420000000000000000000000000000000000000f>.
3. The "events" tab shows when the contract was last upgraded. E.g. in the latest upgrade, the proxy contract was updated to point to the address `0x93e57a196454cb919193fa9946f14943cf733845`.
4. OP uses a proxy contract, so we need to copy the deployed bytecode from the implementation contract address `0x93e57a196454cb919193fa9946f14943cf733845`. This can be found at the bottom of the page: <https://optimistic.etherscan.io/address/0x93e57a196454cb919193fa9946f14943cf733845#code>.
5. Despite OP using a proxy contract, storage is still kept at the predeploys address: `0x420000000000000000000000000000000000000f`. By looking at the implementation contract's source code, we can understand which storage indices are occupied and we can request their values for the proxy contract using a JSON-RPC provider.
