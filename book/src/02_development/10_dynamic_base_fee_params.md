# Dynamic Base Fee Parameters

Since the OP Holocene upgrade, `base_fee_params` are dynamically defined by the `SystemConfig` L1 contract. As a result, each chain may have different parameter values and different activation points.

EDR defines these parameters via chain‑ID–specific configurations. For most OP Stack chains, the configurations are generated automatically. However, for the `op` and `base` chains, the `base_fee_params` are manually overridden to match production values.

Whenever one of the supported chains updates the `SystemConfig` contract with new EIP‑1559 values, the corresponding `base_fee_params` in EDR **must** be updated to remain consistent with the live network.

## How to update dynamic base fee parameters

To determine the dynamic `base_fee_params` for a given chain, follow the steps below.

### 1. Identify the SystemConfig contract

Find the chain’s `SystemConfigProxy` address and `chain_id` in the Superchain Registry:

- [https://github.com/ethereum-optimism/superchain-registry/tree/38c054f1d4179252c91e856fe7115dcbe28962f0/superchain/configs/mainnet](https://github.com/ethereum-optimism/superchain-registry/tree/38c054f1d4179252c91e856fe7115dcbe28962f0/superchain/configs/mainnet)

### 2. Fetch EIP‑1559 parameter update events

Query L1 contract logs for `SystemConfig` updates where `topic2 = 0x4` (EIP‑1559 parameter update).

Replace `apiKey` and `SystemConfigProxyAddress` as appropriate:

```sh
https://api.etherscan.io/v2/api?chainid=1&module=logs&action=getLogs&fromBlock=0&topic0=0x1d2b0bda21d56b8bd12d4f94ebacffdfb35f5e226f84b461103bb8beab6353be&topic0_2_opr=and&topic2=0x0000000000000000000000000000000000000000000000000000000000000004&page=1&offset=0&address=<SystemConfigProxyAddress>&apikey=<apiKey>
```

### 3. Process each log entry

For each log returned:

#### 3.1 Extract the timestamp

Retrieve the timestamp associated with the log entry.

#### 3.2 Map the timestamp to an L2 block

Find the closest L2 block _after_ the timestamp.

Replace `apiKey`, `chainId`, and `timestamp`:

```sh
https://api.etherscan.io/v2/api?apikey=<apiKey>&chainid=<chainId>&module=block&action=getblocknobytime&timestamp=<timestamp>&closest=after
```

#### 3.3 Determine the activation block

Starting from the L2 `blockNumber` returned in step 3.2, search forward through **L2 blockchain blocks** for the first block whose `extra_data` field contains the base fee parameters from the `ConfigUpdate` event.

> ⚠️ **Note**: At the time of writing, there is no deterministic or automated way to locate this block. Manual inspection of subsequent L2 blocks is required.

Fetch L2 block data using:

Replace `apiKey`, `chainId`, and `blockNumber`:

```sh
https://api.etherscan.io/v2/api?apikey=<apiKey>&chainid=<chainId>&module=proxy&action=eth_getBlockByNumber&tag=<blockNumber>&boolean=true
```

##### Activation point logic in EDR

- Let `n` be the **first** block where the correct `extra_data` appears.
- In EDR, the activation point must be set to `blockNumber = n + 1`.

This is required because EDR applies base fee parameters when calculating the gas price using **parent block** information.

### 4. Update EDR configuration

Update the corresponding chain configuration in:

```
crates/edr_op/src/hardfork/<chain>
```

### 5. Add validation tests

Ensure the new activation point and parameter values match the observed on‑chain data. Add a test in `crates/edr_op/tests/integration/dynamic_base_fee_params.rs` that validates the new activation point.
