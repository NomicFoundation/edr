import { toBytes } from "@nomicfoundation/ethereumjs-util";
import chai, { assert } from "chai";
import chaiAsPromised from "chai-as-promised";

import {
  Account,
  GENERIC_CHAIN_TYPE,
  genericChainProviderFactory,
  l1GenesisState,
  l1HardforkFromString,
  l1HardforkLatest,
  l1HardforkToString,
  MineOrdering,
  SubscriptionEvent,
} from "..";
import {
  collectMessages,
  collectSteps,
  ALCHEMY_URL,
  getContext,
} from "./helpers";

chai.use(chaiAsPromised);

describe("Provider", () => {
  const context = getContext();

  before(async () => {
    await context.registerProviderFactory(
      GENERIC_CHAIN_TYPE,
      genericChainProviderFactory()
    );
  });

  const genesisState: Account[] = [
    {
      address: toBytes("0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"),
      balance: 1000n * 10n ** 18n,
      nonce: 0n,
      storage: [],
    },
  ];

  const providerConfig = {
    allowBlocksWithSameTimestamp: false,
    allowUnlimitedContractSize: true,
    bailOnCallFailure: false,
    bailOnTransactionFailure: false,
    blockGasLimit: 300_000_000n,
    chainId: 123n,
    chainOverrides: [],
    coinbase: Buffer.from("0000000000000000000000000000000000000000", "hex"),
    genesisState,
    hardfork: l1HardforkToString(l1HardforkLatest()),
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
    observability: {},
    ownedAccounts: [
      "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80",
    ],
    precompileOverrides: [],
  };

  const loggerConfig = {
    enable: false,
    decodeConsoleLogInputsCallback: (_inputs: ArrayBuffer[]): string[] => {
      return [];
    },
    printLineCallback: (_message: string, _replace: boolean) => {},
  };

  it("initialize local generic provider", async function () {
    const provider = context.createProvider(
      GENERIC_CHAIN_TYPE,
      {
        ...providerConfig,
        genesisState: providerConfig.genesisState.concat(
          l1GenesisState(l1HardforkFromString(providerConfig.hardfork))
        ),
      },
      loggerConfig,
      {
        subscriptionCallback: (_event: SubscriptionEvent) => {},
      },
      {}
    );

    await assert.isFulfilled(provider);
  });

  it("initialize remote", async function () {
    if (ALCHEMY_URL === undefined) {
      this.skip();
    }

    const provider = context.createProvider(
      GENERIC_CHAIN_TYPE,
      {
        ...providerConfig,
        // TODO: Add support for overriding remote fork state when the local fork is different
        fork: {
          jsonRpcUrl: ALCHEMY_URL,
        },
      },
      loggerConfig,
      {
        subscriptionCallback: (_event: SubscriptionEvent) => {},
      },
      {}
    );

    await assert.isFulfilled(provider);
  });

  describe("verbose mode", function () {
    it("should only include the top of the stack by default", async function () {
      const provider = await context.createProvider(
        GENERIC_CHAIN_TYPE,
        {
          ...providerConfig,
          genesisState: providerConfig.genesisState.concat(
            l1GenesisState(l1HardforkFromString(providerConfig.hardfork))
          ),
        },
        loggerConfig,
        {
          subscriptionCallback: (_event: SubscriptionEvent) => {},
        },
        {}
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
              data: "0x60016002600300",
            },
          ],
        })
      );

      const rawTraces = responseObject.traces;
      assert.lengthOf(rawTraces, 1);

      const trace = rawTraces[0].trace;
      const steps = collectSteps(trace);

      assert.lengthOf(steps, 4);

      assert.deepEqual(steps[0].stack, []);
      assert.deepEqual(steps[1].stack, [1n]);
      assert.deepEqual(steps[2].stack, [2n]);
      assert.deepEqual(steps[3].stack, [3n]);
    });

    it("should only include the whole stack if verbose mode is enabled", async function () {
      const provider = await context.createProvider(
        GENERIC_CHAIN_TYPE,
        {
          ...providerConfig,
          genesisState: providerConfig.genesisState.concat(
            l1GenesisState(l1HardforkFromString(providerConfig.hardfork))
          ),
        },
        loggerConfig,
        {
          subscriptionCallback: (_event: SubscriptionEvent) => {},
        },
        {}
      );

      await provider.setVerboseTracing(true);

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
              data: "0x60016002600300",
            },
          ],
        })
      );

      const rawTraces = responseObject.traces;
      assert.lengthOf(rawTraces, 1);

      const trace = rawTraces[0].trace;
      const steps = collectSteps(trace);

      assert.lengthOf(steps, 4);

      // verbose tracing is enabled, so all steps should have a stack
      assert.isTrue(steps.every((step) => step.stack !== undefined));

      assert.deepEqual(steps[0].stack, []);
      assert.deepEqual(steps[1].stack, [1n]);
      assert.deepEqual(steps[2].stack, [1n, 2n]);
      assert.deepEqual(steps[3].stack, [1n, 2n, 3n]);
    });

    it("should not include memory by default", async function () {
      const provider = await context.createProvider(
        GENERIC_CHAIN_TYPE,
        {
          ...providerConfig,
          genesisState: providerConfig.genesisState.concat(
            l1GenesisState(l1HardforkFromString(providerConfig.hardfork))
          ),
        },
        loggerConfig,
        {
          subscriptionCallback: (_event: SubscriptionEvent) => {},
        },
        {}
      );

      const responseObject = await provider.handleRequest(
        JSON.stringify({
          id: 1,
          jsonrpc: "2.0",
          method: "eth_sendTransaction",
          params: [
            {
              from: "0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266",
              // store 0x000...001 as the first memory word
              // PUSH1 1
              // PUSH0
              // MSTORE
              // STOP
              data: "0x60015f5200",
            },
          ],
        })
      );

      const rawTraces = responseObject.traces;
      assert.lengthOf(rawTraces, 1);

      const trace = rawTraces[0].trace;
      const steps = collectSteps(trace);

      assert.lengthOf(steps, 4);

      // verbose tracing is disabled, so none of the steps should have a stack
      assert.isTrue(steps.every((step) => step.memory === undefined));
    });

    it("should include memory if verbose mode is enabled", async function () {
      const provider = await context.createProvider(
        GENERIC_CHAIN_TYPE,
        {
          ...providerConfig,
          genesisState: providerConfig.genesisState.concat(
            l1GenesisState(l1HardforkFromString(providerConfig.hardfork))
          ),
        },
        loggerConfig,
        {
          subscriptionCallback: (_event: SubscriptionEvent) => {},
        },
        {}
      );

      await provider.setVerboseTracing(true);

      const responseObject = await provider.handleRequest(
        JSON.stringify({
          id: 1,
          jsonrpc: "2.0",
          method: "eth_sendTransaction",
          params: [
            {
              from: "0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266",
              // store 0x000...001 as the first memory word
              // PUSH1 1
              // PUSH0
              // MSTORE
              // STOP
              data: "0x60015f5200",
            },
          ],
        })
      );

      const rawTraces = responseObject.traces;
      assert.lengthOf(rawTraces, 1);

      const trace = rawTraces[0].trace;
      const steps = collectSteps(trace);

      assert.lengthOf(steps, 4);

      assertEqualMemory(steps[0].memory, Uint8Array.from([]));
      assertEqualMemory(steps[1].memory, Uint8Array.from([]));
      assertEqualMemory(steps[2].memory, Uint8Array.from([]));
      assertEqualMemory(
        steps[3].memory,
        Uint8Array.from([...Array(31).fill(0), 1])
      );
    });

    it("should include isStaticCall flag in tracing messages", async function () {
      const provider = await context.createProvider(
        GENERIC_CHAIN_TYPE,
        {
          ...providerConfig,
          genesisState: providerConfig.genesisState.concat(
            l1GenesisState(l1HardforkFromString(providerConfig.hardfork))
          ),
        },
        loggerConfig,
        {
          subscriptionCallback: (_event: SubscriptionEvent) => {},
        },
        {}
      );

      const responseObject = await provider.handleRequest(
        JSON.stringify({
          id: 1,
          jsonrpc: "2.0",
          method: "eth_sendTransaction",
          params: [
            {
              from: "0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266",
              // make a static call to the zero address
              // yul: staticcall(gas(), 0, 0, 0, 0, 0)
              data: "0x6000808080805afa00",
              gas: "0x" + 1_000_000n.toString(16),
            },
          ],
        })
      );

      const rawTraces = responseObject.traces;
      assert.lengthOf(rawTraces, 1);

      const trace = rawTraces[0].trace;
      const messageResults = collectMessages(trace);
      assert.lengthOf(messageResults, 2);

      // outer message
      assert.isFalse(messageResults[0].isStaticCall);

      // inner message triggered by STATICCALL
      assert.isTrue(messageResults[1].isStaticCall);
    });

    it("should have tracing information when debug_traceTransaction is used", async function () {
      const provider = await context.createProvider(
        GENERIC_CHAIN_TYPE,
        {
          ...providerConfig,
          genesisState: providerConfig.genesisState.concat(
            l1GenesisState(l1HardforkFromString(providerConfig.hardfork))
          ),
        },
        loggerConfig,
        {
          subscriptionCallback: (_event: SubscriptionEvent) => {},
        },
        {}
      );

      const sendTxResponse = await provider.handleRequest(
        JSON.stringify({
          id: 1,
          jsonrpc: "2.0",
          method: "eth_sendTransaction",
          params: [
            {
              from: "0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266",
              // PUSH1 0x42
              // PUSH0
              // MSTORE
              // PUSH1 0x20
              // PUSH0
              // RETURN
              data: "0x60425f5260205ff3",
              gas: "0x" + 1_000_000n.toString(16),
            },
          ],
        })
      );

      let responseData;

      if (typeof sendTxResponse.data === "string") {
        responseData = JSON.parse(sendTxResponse.data);
      } else {
        responseData = sendTxResponse.data;
      }

      const txHash = responseData.result;

      const traceTransactionResponse = await provider.handleRequest(
        JSON.stringify({
          id: 1,
          jsonrpc: "2.0",
          method: "debug_traceTransaction",
          params: [txHash],
        })
      );

      const rawTraces = traceTransactionResponse.traces;
      assert.lengthOf(rawTraces, 1);
    });

    it("should have tracing information when debug_traceCall is used", async function () {
      const provider = await context.createProvider(
        GENERIC_CHAIN_TYPE,
        {
          ...providerConfig,
          genesisState: providerConfig.genesisState.concat(
            l1GenesisState(l1HardforkFromString(providerConfig.hardfork))
          ),
        },
        loggerConfig,
        {
          subscriptionCallback: (_event: SubscriptionEvent) => {},
        },
        {}
      );

      const traceCallResponse = await provider.handleRequest(
        JSON.stringify({
          id: 1,
          jsonrpc: "2.0",
          method: "debug_traceCall",
          params: [
            {
              from: "0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266",
              // PUSH1 0x42
              // PUSH0
              // MSTORE
              // PUSH1 0x20
              // PUSH0
              // RETURN
              data: "0x60425f5260205ff3",
              gas: "0x" + 1_000_000n.toString(16),
            },
          ],
        })
      );

      const rawTraces = traceCallResponse.traces;
      assert.lengthOf(rawTraces, 1);
    });
  });
});

function assertEqualMemory(
  stepMemory: Uint8Array | undefined,
  expected: Uint8Array
) {
  if (stepMemory === undefined) {
    assert.fail("step memory is undefined");
  }

  assert.deepEqual(stepMemory, expected);
}
