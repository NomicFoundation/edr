import { assert } from "chai";
import { createHardhatNetworkProvider } from "hardhat/internal/hardhat-network/provider/provider";

import { DEFAULT_ACCOUNTS } from "../helpers/providers";

interface VmEventCounts {
  steps: number;
  beforeMessages: number;
  afterMessages: number;
}

async function sendTransactionCountingVmEvents(
  data: string
): Promise<VmEventCounts> {
  const provider: any = await createHardhatNetworkProvider(
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

  const [sender] = (await provider.request({
    method: "eth_accounts",
  })) as string[];

  await provider.request({
    method: "eth_sendTransaction",
    params: [{ from: sender, data, gas: "0x100000" }],
  });

  // The minimal vm uses AsyncEventEmitter: listeners run on a later tick
  await new Promise((resolve) => setImmediate(resolve));

  return counts;
}

describe("Trace-event bridge", function () {
  it("emits step/beforeMessage/afterMessage on a transaction", async function () {
    // Init code executing exactly 4 steps: PUSH1 1, PUSH1 2, PUSH1 3, STOP
    const counts = await sendTransactionCountingVmEvents("0x60016002600300");

    assert.equal(counts.beforeMessages, 1, "beforeMessage events");
    assert.equal(counts.afterMessages, 1, "afterMessage events");
    assert.equal(counts.steps, 4, "step events");
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
});
