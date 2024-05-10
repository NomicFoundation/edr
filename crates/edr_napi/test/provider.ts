import chai, { assert } from "chai";
import chaiAsPromised from "chai-as-promised";

import {
  ContractAndFunctionName,
  EdrContext,
  MineOrdering,
  Provider,
  SpecId,
  SubscriptionEvent,
} from "..";
import { collectSteps } from "./helpers";

chai.use(chaiAsPromised);

function getEnv(key: string): string | undefined {
  const variable = process.env[key];
  if (variable === undefined || variable === "") {
    return undefined;
  }

  const trimmed = variable.trim();

  return trimmed.length === 0 ? undefined : trimmed;
}

const ALCHEMY_URL = getEnv("ALCHEMY_URL");

describe("Provider", () => {
  const context = new EdrContext();
  const providerConfig = {
    allowBlocksWithSameTimestamp: false,
    allowUnlimitedContractSize: true,
    bailOnCallFailure: false,
    bailOnTransactionFailure: false,
    blockGasLimit: 300_000_000n,
    chainId: 123n,
    chains: [],
    coinbase: Buffer.from("0000000000000000000000000000000000000000", "hex"),
    genesisAccounts: [
      {
        secretKey:
          "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80",
        balance: 1000n * 10n ** 18n,
      },
    ],
    hardfork: SpecId.Latest,
    initialBlobGas: {
      gasUsed: 0n,
      excessGas: 0n,
    },
    initialParentBeaconBlockRoot: Buffer.from(
      "0000000000000000000000000000000000000000000000000000000000000000",
      "hex"
    ),
    minGasPrice: 0n,
    mining: {
      autoMine: true,
      memPool: {
        order: MineOrdering.Priority,
      },
    },
    networkId: 123n,
  };

  const loggerConfig = {
    enable: false,
    decodeConsoleLogInputsCallback: (inputs: Buffer[]): string[] => {
      return [];
    },
    getContractAndFunctionNameCallback: (
      _code: Buffer,
      _calldata?: Buffer
    ): ContractAndFunctionName => {
      return {
        contractName: "",
      };
    },
    printLineCallback: (message: string, replace: boolean) => {},
  };

  it("initialize local", async function () {
    const provider = Provider.withConfig(
      context,
      providerConfig,
      loggerConfig,
      (_event: SubscriptionEvent) => {}
    );

    await assert.isFulfilled(provider);
  });

  it("initialize remote", async function () {
    if (ALCHEMY_URL === undefined) {
      this.skip();
    }

    const provider = Provider.withConfig(
      context,
      {
        fork: {
          jsonRpcUrl: ALCHEMY_URL,
        },
        ...providerConfig,
      },
      loggerConfig,
      (_event: SubscriptionEvent) => {}
    );

    await assert.isFulfilled(provider);
  });

  describe("verbose mode", function () {
    it("should only include the top of the stack by default", async function () {
      const provider = await Provider.withConfig(
        context,
        providerConfig,
        loggerConfig,
        (_event: SubscriptionEvent) => {}
      );

      const responseObject = await provider.handleRequest(
        JSON.stringify({
          id: 1,
          jsonrpc: "2.0",
          method: "eth_sendTransaction",
          params: [
            {
              from: "0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266",
              // PUSH1 1
              // PUSH1 2
              // PUSH1 3
              // STOP
              data: "60016002600300",
            },
          ],
        })
      );

      const rawTraces = responseObject.traces;
      assert.lengthOf(rawTraces, 1);

      const trace = rawTraces[0].trace();
      const steps = collectSteps(trace);

      assert.lengthOf(steps, 4);

      // verbose tracing is disabled, so none of the steps should have a stack
      assert.isTrue(steps.every((step) => step.stack === undefined));

      assert.isUndefined(steps[0].stackTop);
      assert.equal(steps[1].stackTop, 1n);
      assert.equal(steps[2].stackTop, 2n);
      assert.equal(steps[3].stackTop, 3n);
    });

    it("should only include the whole stack if verbose mode is enabled", async function () {
      const provider = await Provider.withConfig(
        context,
        providerConfig,
        loggerConfig,
        (_event: SubscriptionEvent) => {}
      );

      provider.setVerboseTracing(true);

      const responseObject = await provider.handleRequest(
        JSON.stringify({
          id: 1,
          jsonrpc: "2.0",
          method: "eth_sendTransaction",
          params: [
            {
              from: "0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266",
              // PUSH1 1
              // PUSH1 2
              // PUSH1 3
              // STOP
              data: "60016002600300",
            },
          ],
        })
      );

      const rawTraces = responseObject.traces;
      assert.lengthOf(rawTraces, 1);

      const trace = rawTraces[0].trace();
      const steps = collectSteps(trace);

      assert.lengthOf(steps, 4);

      // verbose tracing is enabled, so all steps should have a stack
      assert.isTrue(steps.every((step) => step.stack !== undefined));

      // same assertions as when verbose tracing is disabled
      assert.isUndefined(steps[0].stackTop);
      assert.equal(steps[1].stackTop, 1n);
      assert.equal(steps[2].stackTop, 2n);
      assert.equal(steps[3].stackTop, 3n);

      assert.deepEqual(steps[0].stack, []);
      assert.deepEqual(steps[1].stack, [1n]);
      assert.deepEqual(steps[2].stack, [1n, 2n]);
      assert.deepEqual(steps[3].stack, [1n, 2n, 3n]);
    });
  });
});
