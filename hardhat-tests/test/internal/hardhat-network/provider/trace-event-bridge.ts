import { assert } from "chai";
import { createHardhatNetworkProvider } from "hardhat/internal/hardhat-network/provider/provider";

import { DEFAULT_ACCOUNTS } from "../helpers/providers";

describe("Trace-event bridge", function () {
  it("emits step/beforeMessage/afterMessage on a transaction", async function () {
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

    let steps = 0;
    let beforeMessages = 0;
    let afterMessages = 0;
    provider._node._vm.evm.events.on("step", () => {
      steps += 1;
    });
    provider._node._vm.evm.events.on("beforeMessage", () => {
      beforeMessages += 1;
    });
    provider._node._vm.evm.events.on("afterMessage", () => {
      afterMessages += 1;
    });

    const [sender] = (await provider.request({
      method: "eth_accounts",
    })) as string[];

    // Init code executing exactly 4 steps: PUSH1 1, PUSH1 2, PUSH1 3, STOP
    await provider.request({
      method: "eth_sendTransaction",
      params: [{ from: sender, data: "0x60016002600300", gas: "0x100000" }],
    });

    // The minimal vm uses AsyncEventEmitter: listeners run on a later tick
    await new Promise((resolve) => setImmediate(resolve));

    assert.equal(beforeMessages, 1, "beforeMessage events");
    assert.equal(afterMessages, 1, "afterMessage events");
    assert.equal(steps, 4, "step events");
  });
});
