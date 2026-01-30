import { toBytes } from "@nomicfoundation/ethereumjs-util";
import chai, { assert } from "chai";
import chaiAsPromised from "chai-as-promised";
import { Interface } from "ethers";

import {
  AccountOverride,
  ContractDecoder,
  GENERIC_CHAIN_TYPE,
  genericChainProviderFactory,
  l1GenesisState,
  l1HardforkFromString,
  l1HardforkLatest,
  l1HardforkToString,
  MineOrdering,
  SubscriptionEvent,
  precompileP256Verify,
  OP_CHAIN_TYPE,
  opProviderFactory,
  opHardforkToString,
  OpHardfork,
  SpecId,
} from "..";
import {
  collectMessages,
  collectSteps,
  ALCHEMY_URL,
  getContext,
  loadContract,
} from "./helpers";

chai.use(chaiAsPromised);

describe("Provider", () => {
  const context = getContext();

  before(async () => {
    await context.registerProviderFactory(
      GENERIC_CHAIN_TYPE,
      genericChainProviderFactory()
    );
    await context.registerProviderFactory(OP_CHAIN_TYPE, opProviderFactory());
  });

  const genesisAddress = "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266";
  const genesisState: AccountOverride[] = [
    {
      address: toBytes(genesisAddress),
      balance: 1000n * 10n ** 18n,
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
    coinbase: new Uint8Array(
      Buffer.from("0000000000000000000000000000000000000000", "hex")
    ),
    genesisState,
    hardfork: l1HardforkToString(l1HardforkLatest()),
    initialBlobGas: {
      gasUsed: 0n,
      excessGas: 0n,
    },
    initialParentBeaconBlockRoot: new Uint8Array(
      Buffer.from(
        "0000000000000000000000000000000000000000000000000000000000000000",
        "hex"
      )
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
      new ContractDecoder()
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
          url: ALCHEMY_URL,
        },
      },
      loggerConfig,
      {
        subscriptionCallback: (_event: SubscriptionEvent) => {},
      },
      new ContractDecoder()
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
        new ContractDecoder()
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
        new ContractDecoder()
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
        new ContractDecoder()
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
        new ContractDecoder()
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
        new ContractDecoder()
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
        new ContractDecoder()
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
        new ContractDecoder()
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

  async function deployAndTestCustomPrecompile(enabled: boolean) {
    // Contract code in edr/data/contracts/CustomPrecompile.sol
    const contractArtifact = loadContract(
      "./data/artifacts/default/CustomPrecompile.json"
    );
    const contractInterface = new Interface(contractArtifact.contract.abi);

    const provider = await context.createProvider(
      GENERIC_CHAIN_TYPE,
      {
        ...providerConfig,
        genesisState: providerConfig.genesisState.concat(
          l1GenesisState(l1HardforkFromString(providerConfig.hardfork))
        ),
        // Use a pre-Osaka hardfork to ensure the precompile is not available by default
        hardfork: l1HardforkToString(SpecId.Prague),
        ...(enabled ? { precompileOverrides: [precompileP256Verify()] } : {}),
      },
      loggerConfig,
      {
        subscriptionCallback: (_event: SubscriptionEvent) => {},
      },
      new ContractDecoder()
    );

    const sender = "0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266";

    const deploymentTransactionResponse = await provider.handleRequest(
      JSON.stringify({
        id: 1,
        jsonrpc: "2.0",
        method: "eth_sendTransaction",
        params: [
          {
            from: sender,
            data: contractArtifact.contract.bytecode,
          },
        ],
      })
    );

    const deploymentTransactionHash = JSON.parse(
      deploymentTransactionResponse.data
    ).result;

    const deploymentTransactionReceiptResponse = await provider.handleRequest(
      JSON.stringify({
        id: 1,
        jsonrpc: "2.0",
        method: "eth_getTransactionReceipt",
        params: [deploymentTransactionHash],
      })
    );

    const deployedAddress = JSON.parse(
      deploymentTransactionReceiptResponse.data
    ).result.contractAddress;

    const precompileTransactionResponse = await provider.handleRequest(
      JSON.stringify({
        id: 1,
        jsonrpc: "2.0",
        method: "eth_sendTransaction",
        params: [
          {
            from: sender,
            to: deployedAddress,
            data: contractInterface.encodeFunctionData("rip7212Precompile"),
          },
        ],
      })
    );

    const precompileTransactionHash = JSON.parse(
      precompileTransactionResponse.data
    ).result;

    const precompileTransactionReceiptResponse = await provider.handleRequest(
      JSON.stringify({
        id: 1,
        jsonrpc: "2.0",
        method: "eth_getTransactionReceipt",
        params: [precompileTransactionHash],
      })
    );

    return JSON.parse(precompileTransactionReceiptResponse.data).result;
  }

  it("custom precompile enabled", async function () {
    const precompileReceipt = await deployAndTestCustomPrecompile(true);
    assert.strictEqual(precompileReceipt.status, "0x1");
  });

  it("custom precompile disabled", async function () {
    const precompileReceipt = await deployAndTestCustomPrecompile(false);
    assert.strictEqual(precompileReceipt.status, "0x0");
  });

  it("allows baseFeeConfig configuration", async function () {
    const provider = await context.createProvider(
      OP_CHAIN_TYPE,
      {
        ...providerConfig,
        hardfork: opHardforkToString(OpHardfork.Holocene),
        baseFeeConfig: [
          {
            activation: { blockNumber: BigInt(0) },
            maxChangeDenominator: BigInt(50),
            elasticityMultiplier: BigInt(6),
          },
          {
            activation: { hardfork: opHardforkToString(OpHardfork.Canyon) },
            maxChangeDenominator: BigInt(250),
            elasticityMultiplier: BigInt(6),
          },
          {
            activation: { blockNumber: BigInt(135_513_416) },
            maxChangeDenominator: BigInt(250),
            elasticityMultiplier: BigInt(4),
          },
        ],
      },
      loggerConfig,
      {
        subscriptionCallback: (_event: SubscriptionEvent) => {},
      },
      new ContractDecoder()
    );

    await provider.handleRequest(
      JSON.stringify({
        id: 1,
        jsonrpc: "2.0",
        method: "eth_sendTransaction",
        params: [
          {
            from: "0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266",
            to: "420000000000000000000000000000000000000F",
            data: "de26c4a1",
          },
        ],
      })
    );
    const block = await provider.handleRequest(
      JSON.stringify({
        id: 1,
        jsonrpc: "2.0",
        method: "eth_getBlockByNumber",
        params: ["latest", false],
      })
    );
    const responseData = JSON.parse(block.data);
    const lastBlockExtraData = responseData.result.extraData;

    const bytes = new Uint8Array(
      Buffer.from(lastBlockExtraData.split("0x")[1], "hex")
    );
    const dataView = new DataView(bytes.buffer);
    const extraDataVersionByte = 0;
    const denominatorLeastSignificantByte = 4;
    const elasticityLeastSignificantByte = 8;

    assert.equal(0, dataView.getUint8(extraDataVersionByte));
    // we are expecting base_fee_params associated to Canyon activation point (250,6) since provider was created
    // with Holocene hardfork, which is after Canyon
    assert.equal(250, dataView.getUint8(denominatorLeastSignificantByte));
    assert.equal(6, dataView.getUint8(elasticityLeastSignificantByte));
  });

  describe("eth_getProof", () => {
    it("encodes an error within data when not supported for fork mode", async function () {
      if (ALCHEMY_URL === undefined) {
        this.skip();
      }

      const provider = await context.createProvider(
        GENERIC_CHAIN_TYPE,
        {
          ...providerConfig,
          fork: {
            url: ALCHEMY_URL,
          },
        },
        loggerConfig,
        {
          subscriptionCallback: (_event) => {},
        },
        new ContractDecoder()
      );

      const response = await provider.handleRequest(
        JSON.stringify({
          id: 1,
          jsonrpc: "2.0",
          method: "eth_getProof",
          params: [genesisAddress, [], "latest"],
        })
      );
      const responseData = JSON.parse(response.data);
      assert.include(
        responseData.error.message,
        "The action `Proof of locally modified state in fork mode` is unsupported"
      );
    });

    it("fails on invalid storage key", async function () {
      const provider = await context.createProvider(
        GENERIC_CHAIN_TYPE,
        {
          ...providerConfig,
        },
        loggerConfig,
        {
          subscriptionCallback: (_event) => {},
        },
        new ContractDecoder()
      );

      const storageKey = "b421";

      const response = await provider.handleRequest(
        JSON.stringify({
          id: 1,
          jsonrpc: "2.0",
          method: "eth_getProof",
          params: [genesisAddress, [storageKey], "latest"],
        })
      );
      const INVALID_PARAM_CODE = -32602;
      const responseData = JSON.parse(response.data);
      assert.equal(responseData.error.code, INVALID_PARAM_CODE);
    });

    it("deserializes storage keys correctly", async function () {
      const provider = await context.createProvider(
        GENERIC_CHAIN_TYPE,
        {
          ...providerConfig,
        },
        loggerConfig,
        {
          subscriptionCallback: (_event) => {},
        },
        new ContractDecoder()
      );

      const storageKey =
        "0x56e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421";
      const response = await provider.handleRequest(
        JSON.stringify({
          id: 1,
          jsonrpc: "2.0",
          method: "eth_getProof",
          params: [genesisAddress, [storageKey], "latest"],
        })
      );
      const responseData = JSON.parse(response.data);
      const storageProof = responseData.result.storageProof[0];
      assert.equal(storageProof.key, storageKey);
      assert.equal(storageProof.value, "0x0");
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
