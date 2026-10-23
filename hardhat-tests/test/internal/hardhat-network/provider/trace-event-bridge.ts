import { assert } from "chai";
import { createHardhatNetworkProvider } from "hardhat/internal/hardhat-network/provider/provider";

import { DEFAULT_ACCOUNTS } from "../helpers/providers";

// Init code executing exactly 4 steps: PUSH1 1, PUSH1 2, PUSH1 3, STOP
const FOUR_STEP_INIT_CODE = "0x60016002600300";
const FOUR_STEP_INIT_CODE_STEPS = 4;

interface VmEventCounts {
  steps: number;
  beforeMessages: number;
  afterMessages: number;
}

async function createProvider(): Promise<any> {
  return createHardhatNetworkProvider(
    {
      hardfork: "cancun",
      chainId: 123,
      networkId: 123,
      blockGasLimit: 6000000,
      minGasPrice: 0n,
      throwOnTransactionFailures: true,
      throwOnCallFailures: true,
      automine: true,
      intervalMining: 0,
      mempoolOrder: "priority",
      chains: new Map(),
      genesisAccounts: DEFAULT_ACCOUNTS,
      allowUnlimitedContractSize: false,
      allowBlocksWithSameTimestamp: false,
      enableTransientStorage: false,
      enableRip7212: false,
    },
    { enabled: false }
  );
}

async function sendTransaction(provider: any, data: string): Promise<void> {
  const [sender] = (await provider.request({
    method: "eth_accounts",
  })) as string[];

  await provider.request({
    method: "eth_sendTransaction",
    params: [{ from: sender, data, gas: "0x100000" }],
  });

  // The minimal vm uses AsyncEventEmitter: listeners run on a later tick
  await new Promise((resolve) => setImmediate(resolve));
}

/**
 * Records the number of call traces of each response the wrapper's current
 * EDR provider returns. `hardhat_reset` replaces that provider, so call this
 * again after a reset.
 */
function recordTraceCounts(provider: any): number[] {
  const traceCounts: number[] = [];
  const edrProvider = provider._provider;
  const handleRequest = edrProvider.handleRequest.bind(edrProvider);
  edrProvider.handleRequest = async (request: string) => {
    const response = await handleRequest(request);
    traceCounts.push(response.traces().length);
    return response;
  };
  return traceCounts;
}

async function sendTransactionCountingVmEvents(
  data: string
): Promise<VmEventCounts> {
  const provider = await createProvider();

  const counts: VmEventCounts = {
    steps: 0,
    beforeMessages: 0,
    afterMessages: 0,
  };
  provider._node._vm.evm.events.on("step", () => {
    counts.steps += 1;
  });
  provider._node._vm.evm.events.on("beforeMessage", () => {
    counts.beforeMessages += 1;
  });
  provider._node._vm.evm.events.on("afterMessage", () => {
    counts.afterMessages += 1;
  });

  await sendTransaction(provider, data);

  return counts;
}

describe("Trace-event bridge", function () {
  it("emits step/beforeMessage/afterMessage on a transaction", async function () {
    const counts = await sendTransactionCountingVmEvents(FOUR_STEP_INIT_CODE);

    assert.equal(counts.beforeMessages, 1, "beforeMessage events");
    assert.equal(counts.afterMessages, 1, "afterMessage events");
    assert.equal(counts.steps, FOUR_STEP_INIT_CODE_STEPS, "step events");
  });

  it("emits beforeMessage/afterMessage for a precompile call", async function () {
    // Init code calling the identity precompile at 0x04:
    // PUSH1 0 (x5: return length & offset, args length & offset, value)
    // PUSH1 4, PUSH2 0xffff (gas), CALL, STOP
    const counts = await sendTransactionCountingVmEvents(
      "0x60006000600060006000600461fffff100"
    );

    assert.equal(counts.beforeMessages, 2, "beforeMessage events");
    assert.equal(counts.afterMessages, 2, "afterMessage events");
    assert.equal(counts.steps, 9, "step events");
  });

  it("collects no call traces while no VM listener is registered", async function () {
    const provider = await createProvider();
    const traceCounts = recordTraceCounts(provider);

    await sendTransaction(provider, FOUR_STEP_INIT_CODE);

    assert.deepEqual(traceCounts, [0, 0], "traces per response");
  });

  it("collects call traces only while a VM listener is registered", async function () {
    const provider = await createProvider();
    const traceCounts = recordTraceCounts(provider);

    let steps = 0;
    const onStep = () => {
      steps += 1;
    };

    provider._node._vm.evm.events.on("step", onStep);
    await sendTransaction(provider, FOUR_STEP_INIT_CODE);
    assert.equal(steps, FOUR_STEP_INIT_CODE_STEPS, "steps while registered");

    provider._node._vm.evm.events.off("step", onStep);
    await sendTransaction(provider, FOUR_STEP_INIT_CODE);
    assert.equal(steps, FOUR_STEP_INIT_CODE_STEPS, "steps after removal");

    // eth_accounts, then eth_sendTransaction, per transaction
    assert.deepEqual(traceCounts, [0, 1, 0, 0], "traces per response");
  });

  it("keeps emitting step events for a listener registered across hardhat_reset", async function () {
    const provider = await createProvider();

    let steps = 0;
    provider._node._vm.evm.events.on("step", () => {
      steps += 1;
    });

    await sendTransaction(provider, FOUR_STEP_INIT_CODE);
    await provider.request({ method: "hardhat_reset", params: [] });
    await sendTransaction(provider, FOUR_STEP_INIT_CODE);

    assert.equal(steps, 2 * FOUR_STEP_INIT_CODE_STEPS, "step events");
  });

  it("follows a listener removed before and re-added after hardhat_reset", async function () {
    const provider = await createProvider();

    let steps = 0;
    const onStep = () => {
      steps += 1;
    };

    provider._node._vm.evm.events.on("step", onStep);
    await sendTransaction(provider, FOUR_STEP_INIT_CODE);
    provider._node._vm.evm.events.off("step", onStep);

    await provider.request({ method: "hardhat_reset", params: [] });
    const traceCountsAfterReset = recordTraceCounts(provider);

    await sendTransaction(provider, FOUR_STEP_INIT_CODE);
    assert.equal(steps, FOUR_STEP_INIT_CODE_STEPS, "steps while removed");

    provider._node._vm.evm.events.on("step", onStep);
    await sendTransaction(provider, FOUR_STEP_INIT_CODE);
    assert.equal(steps, 2 * FOUR_STEP_INIT_CODE_STEPS, "steps after re-adding");

    // eth_accounts, then eth_sendTransaction, per transaction
    assert.deepEqual(
      traceCountsAfterReset,
      [0, 0, 0, 1],
      "traces per response after reset"
    );
  });
});
