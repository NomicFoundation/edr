/**
 * Measures the user-visible impact of interval-mining scheduling on
 * read-only requests that are queued while mutating transactions are
 * pending. Everything goes through the public JSON-RPC surface; there are
 * no test hooks.
 *
 * Scenario:
 *   1. auto-mine off; deploy a gas-burner contract (its 7-byte runtime is an
 *      unconditional loop hashing the empty slice, so a call or transaction
 *      burns all the gas it is given).
 *   2. queue TX_COUNT heavy transactions; `blockGasLimit` fits exactly one
 *      per block, so each mining pass has real work to do.
 *   3. enable interval mining (INTERVAL_MS).
 *   4. immediately fire CALL_COUNT concurrent compute-heavy `eth_call`s
 *      (read-only) plus CHEAP_COUNT trivial `eth_getBalance`s, and measure
 *      each response's latency individually. Mined blocks are timestamped
 *      via a `newHeads` subscription.
 *
 * On the event-loop design (post-#1486), a mining pass that outlasts the
 * interval is due again before the next queued request is dequeued, so reads
 * drain roughly one per pass (latency staircase). On `main`, the fair data
 * mutex serves all queued reads before the mining task's next turn (flat
 * latencies, delayed blocks).
 *
 * Usage:
 *   pnpm -C crates/edr_napi build:dev     # release build of the napi addon
 *   node read-latency-under-interval-mining.ts                # 100ms interval
 *   node read-latency-under-interval-mining.ts --interval=1   # legal minimum
 *   node read-latency-under-interval-mining.ts --no-interval  # baseline
 *
 * Needs node >= 22.18 (built-in type stripping). To compare designs, run it
 * once per branch, rebuilding the napi addon in between (or point
 * EDR_NAPI_PATH at a checkout that has a built addon).
 */

import { createRequire } from "node:module";

const require = createRequire(import.meta.url);

const EDR_NAPI_PATH =
  process.env.EDR_NAPI_PATH ??
  new URL("../../crates/edr_napi", import.meta.url).pathname;

const {
  ContractDecoder,
  EdrContext,
  GENERIC_CHAIN_TYPE,
  genericChainProviderFactory,
  l1GenesisState,
  l1HardforkFromString,
  l1HardforkLatest,
  l1HardforkToString,
  MineOrdering,
} = require(EDR_NAPI_PATH);

// --- parameters -----------------------------------------------------------

const argv = process.argv.slice(2);
const intervalArg = argv.find((a: string) => a.startsWith("--interval="));
const INTERVAL_MS = argv.includes("--no-interval")
  ? 0
  : intervalArg
    ? Number(intervalArg.split("=")[1])
    : 100;

const txArg = argv.find((a: string) => a.startsWith("--txs="));
const TX_COUNT = txArg ? Number(txArg.split("=")[1]) : 6; // heavy pending txs
const CALL_COUNT = 6; // concurrent compute-heavy eth_calls
const CHEAP_COUNT = 3; // trivial reads queued after the calls
const TX_GAS = 5_500_000; // per heavy transaction
const CALL_GAS = 2_000_000; // per eth_call
const BLOCK_GAS_LIMIT = 6_000_000n; // fits exactly one heavy tx per block

// Deploys runtime `5b 5f 5f 20 50 5f 56`: an unconditional loop hashing the
// empty slice, so any call/transaction burns all the gas it is given.
const GAS_BURNER_INIT_CODE = "0x665b5f5f20505f565f5260076019f3";

const OWNED_SECRET_KEY =
  "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";
const SENDER = "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266";

// --- JSON-RPC plumbing ----------------------------------------------------

let nextId = 1;

async function rpc(provider: any, method: string, params: any[] = []) {
  const responseObject = await provider.handleRequest(
    JSON.stringify({ id: nextId++, jsonrpc: "2.0", method, params })
  );
  const data = responseObject.data;
  return typeof data === "string" ? JSON.parse(data) : data;
}

function quantity(n: number | bigint): string {
  return `0x${n.toString(16)}`;
}

async function main() {
  const context = new EdrContext();
  await context.registerProviderFactory(
    GENERIC_CHAIN_TYPE,
    genericChainProviderFactory()
  );

  let clock = performance.now();
  const elapsed = () => (performance.now() - clock).toFixed(0).padStart(6);

  const hardfork = l1HardforkToString(l1HardforkLatest());
  const provider = await context.createProvider(
    GENERIC_CHAIN_TYPE,
    {
      allowBlocksWithSameTimestamp: true,
      allowUnlimitedContractSize: true,
      bailOnCallFailure: false,
      bailOnTransactionFailure: false,
      chainId: 123n,
      coinbase: new Uint8Array(20),
      defaultTransactionGasLimit: 300_000n,
      genesisState: [
        {
          address: Buffer.from(SENDER.slice(2), "hex"),
          balance: 10_000n * 10n ** 18n,
        },
        ...l1GenesisState(l1HardforkFromString(hardfork)),
      ],
      hardfork,
      initialParentBeaconBlockRoot: new Uint8Array(32),
      minGasPrice: 0n,
      mining: {
        autoMine: true, // switched off after the deploy
        blockGasLimit: BLOCK_GAS_LIMIT,
        memPool: { order: MineOrdering.Priority },
      },
      network: {
        genesisBlobGas: { gasUsed: 0n, excessGas: 0n },
        genesisBlockGasLimit: BLOCK_GAS_LIMIT,
      },
      networkId: 123n,
      observability: {},
      ownedAccounts: [OWNED_SECRET_KEY],
      precompileOverrides: [],
    },
    {
      enable: false,
      decodeConsoleLogInputsCallback: () => [],
      printLineCallback: () => {},
    },
    {
      subscriptionCallback: argv.includes("--log-blocks")
        ? () => {
            console.log(`  block mined   at ${elapsed()} ms`);
          }
        : () => {},
    },
    new ContractDecoder()
  );

  // 1. Deploy the gas burner while auto-mine is still on.
  const deployResponse = await rpc(provider, "eth_sendTransaction", [
    { from: SENDER, data: GAS_BURNER_INIT_CODE, gas: quantity(1_000_000) },
  ]);
  const receipt = await rpc(provider, "eth_getTransactionReceipt", [
    deployResponse.result,
  ]);
  const burner = receipt.result.contractAddress;

  await rpc(provider, "evm_setAutomine", [false]);
  await rpc(provider, "eth_subscribe", ["newHeads"]);

  // Warm up the process (JIT, caches) so the first measured call is not an
  // outlier, and measure the warm per-call cost.
  await rpc(provider, "eth_call", [
    { from: SENDER, to: burner, gas: quantity(CALL_GAS) },
    "latest",
  ]);
  const warmStart = performance.now();
  await rpc(provider, "eth_call", [
    { from: SENDER, to: burner, gas: quantity(CALL_GAS) },
    "latest",
  ]);
  console.log(
    `warm eth_call of ${CALL_GAS} gas costs ~${(
      performance.now() - warmStart
    ).toFixed(0)} ms`
  );

  // 2. Queue the heavy transactions; they stay pending (auto-mine is off).
  for (let i = 0; i < TX_COUNT; i++) {
    const response = await rpc(provider, "eth_sendTransaction", [
      { from: SENDER, to: burner, gas: quantity(TX_GAS) },
    ]);
    if (response.error) {
      throw new Error(`queueing tx ${i} failed: ${response.error.message}`);
    }
  }
  const blocksBefore = Number((await rpc(provider, "eth_blockNumber")).result);

  // 3. Enable interval mining (unless measuring the baseline).
  if (INTERVAL_MS > 0) {
    await rpc(provider, "evm_setIntervalMining", [INTERVAL_MS]);
  }

  // 4. Fire the reads concurrently and time each one.
  const started = performance.now();
  clock = started; // block-mined log lines share the same origin
  const timed = (label: string, promise: Promise<any>) =>
    promise.then((response) => {
      const ms = performance.now() - started;
      console.log(
        `${label.padEnd(14)} answered at ${ms.toFixed(0).padStart(6)} ms`
      );
      return response;
    });

  const reads: Promise<any>[] = [];
  for (let i = 0; i < CALL_COUNT; i++) {
    reads.push(
      timed(
        `eth_call #${i + 1}`,
        rpc(provider, "eth_call", [
          { from: SENDER, to: burner, gas: quantity(CALL_GAS) },
          "latest",
        ])
      )
    );
  }
  for (let i = 0; i < CHEAP_COUNT; i++) {
    reads.push(
      timed(
        `getBalance #${i + 1}`,
        rpc(provider, "eth_getBalance", [SENDER, "latest"])
      )
    );
  }
  await Promise.all(reads);
  const total = performance.now() - started;

  // Let pending blocks finish, then report.
  if (INTERVAL_MS > 0) {
    await new Promise((resolve) => setTimeout(resolve, INTERVAL_MS * 2 + 500));
    await rpc(provider, "evm_setIntervalMining", [0]);
  }
  const blocksAfter = Number((await rpc(provider, "eth_blockNumber")).result);
  for (
    let n = blocksBefore + 1;
    n <= Math.min(blocksAfter, blocksBefore + 8);
    n++
  ) {
    const block = (
      await rpc(provider, "eth_getBlockByNumber", [quantity(n), false])
    ).result;
    console.log(
      `block ${n}: ${block.transactions.length} txs, gasUsed ${Number(block.gasUsed)}`
    );
  }

  console.log("---");
  console.log(
    `interval: ${INTERVAL_MS} ms | ${TX_COUNT} pending txs of ${TX_GAS} gas ` +
      `(1 per block) | ${CALL_COUNT} eth_calls of ${CALL_GAS} gas + ${CHEAP_COUNT} getBalance`
  );
  console.log(
    `all reads answered in ${total.toFixed(0)} ms; blocks mined meanwhile: ` +
      `${blocksAfter - blocksBefore}`
  );
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
