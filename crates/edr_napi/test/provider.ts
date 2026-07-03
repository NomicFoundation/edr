import { toBytes } from "@nomicfoundation/ethereumjs-util";
import chai, { assert } from "chai";
import chaiAsPromised from "chai-as-promised";
import { Interface } from "ethers";

import {
  AccountOverride,
  CallOverrideResult,
  ContractDecoder,
  GENERIC_CHAIN_TYPE,
  genericChainProviderFactory,
  IncludeTraces,
  l1GenesisState,
  l1HardforkFromString,
  l1HardforkLatest,
  l1HardforkToString,
  MineOrdering,
  Provider,
  SubscriptionEvent,
  precompileP256Verify,
  OP_CHAIN_TYPE,
  opProviderFactory,
  opHardforkToString,
  OpHardfork,
  SpecId,
} from "..";
import {
  ALCHEMY_URL,
  collectMessages,
  collectSteps,
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
    chainId: 123n,
    chainOverrides: [],
    coinbase: new Uint8Array(
      Buffer.from("0000000000000000000000000000000000000000", "hex")
    ),
    defaultTransactionGasLimit: 300_000_000n,
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
      blockGasLimit: 300_000_000n,
      memPool: {
        order: MineOrdering.Priority,
      },
    },
    network: {
      genesisBlobGas: {
        gasUsed: 0n,
        excessGas: 0n,
      },
      genesisBlockGasLimit: 300_000_000n,
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

  // Used by the callback tests below.
  async function createGenericProvider(
    logger: typeof loggerConfig,
    subscriptionCallback: (event: SubscriptionEvent) => void = () => {}
  ): Promise<Provider> {
    return context.createProvider(
      GENERIC_CHAIN_TYPE,
      {
        ...providerConfig,
        genesisState: providerConfig.genesisState.concat(
          l1GenesisState(l1HardforkFromString(providerConfig.hardfork))
        ),
      },
      logger,
      { subscriptionCallback },
      new ContractDecoder()
    );
  }

  // console.log("hello") calldata and the "console.log" address it targets.
  const CONSOLE_LOG_ADDRESS = "0x000000000000000000636f6e736f6c652e6c6f67";
  const CONSOLE_LOG_HELLO_CALLDATA =
    "0x41304fac" +
    "0000000000000000000000000000000000000000000000000000000000000020" +
    "0000000000000000000000000000000000000000000000000000000000000005" +
    "68656c6c6f000000000000000000000000000000000000000000000000000000";

  // defaultTransactionGasLimit (300M) exceeds the EIP-7825 Osaka cap; set an
  // explicit sub-cap gas.
  const GAS_BELOW_OSAKA_CAP = "0xf4240";

  async function sendConsoleLogHello(provider: Provider): Promise<any> {
    const response = await provider.handleRequest(
      JSON.stringify({
        id: 1,
        jsonrpc: "2.0",
        method: "eth_sendTransaction",
        params: [
          {
            from: genesisAddress,
            to: CONSOLE_LOG_ADDRESS,
            data: CONSOLE_LOG_HELLO_CALLDATA,
            gas: GAS_BELOW_OSAKA_CAP,
          },
        ],
      })
    );
    return JSON.parse(response.data);
  }

  it("initialize local generic provider", async function () {
    await assert.isFulfilled(createGenericProvider(loggerConfig));
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
        network: {
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
    const tracingProviderConfig = {
      ...providerConfig,
      genesisState: providerConfig.genesisState.concat(
        l1GenesisState(l1HardforkFromString(providerConfig.hardfork))
      ),
      observability: {
        includeCallTraces: IncludeTraces.All,
      },
    };

    it("should only include the top of the stack by default", async function () {
      const provider = await context.createProvider(
        GENERIC_CHAIN_TYPE,
        tracingProviderConfig,
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
              gas: "0x" + 1_000_000n.toString(16),
            },
          ],
        })
      );

      const rawTraces = responseObject.traces();
      assert.lengthOf(rawTraces, 1);

      const trace = rawTraces[0];
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
        tracingProviderConfig,
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
              gas: "0x" + 1_000_000n.toString(16),
            },
          ],
        })
      );

      const rawTraces = responseObject.traces();
      assert.lengthOf(rawTraces, 1);

      const trace = rawTraces[0];
      const steps = collectSteps(trace);

      assert.lengthOf(steps, 4);

      // verbose tracing is enabled, so all steps should have a stack
      assert.isTrue(steps.every((step) => step.stack !== undefined));

      assert.deepEqual(steps[0].stack, []);
      assert.deepEqual(steps[1].stack, [1n]);
      assert.deepEqual(steps[2].stack, [1n, 2n]);
      assert.deepEqual(steps[3].stack, [1n, 2n, 3n]);
    });

    it("should include the top of the stack across nested call frames", async function () {
      const provider = await context.createProvider(
        GENERIC_CHAIN_TYPE,
        tracingProviderConfig,
        loggerConfig,
        {
          subscriptionCallback: (_event: SubscriptionEvent) => {},
        },
        new ContractDecoder()
      );

      // Deploy a contract with runtime code:
      // PUSH1 0x0a
      // PUSH1 0x0b
      // STOP
      await provider.handleRequest(
        JSON.stringify({
          id: 1,
          jsonrpc: "2.0",
          method: "eth_sendTransaction",
          params: [
            {
              from: "0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266",
              // PUSH5 0x600a600b00
              // PUSH0
              // MSTORE
              // PUSH1 5 (length)
              // PUSH1 27 (offset)
              // RETURN
              data: "0x64600a600b005f526005601bf3",
              gas: "0x" + 1_000_000n.toString(16),
            },
          ],
        })
      );

      const calleeAddress = 0x5fbdb2315678afecb367f032d93f642f64180aa3n;

      const responseObject = await provider.handleRequest(
        JSON.stringify({
          id: 2,
          jsonrpc: "2.0",
          method: "eth_sendTransaction",
          params: [
            {
              from: "0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266",
              // PUSH1 0 (x5: return length & offset, args length & offset, value)
              // PUSH20 <callee address>
              // PUSH2 0xffff (gas)
              // CALL
              // PUSH1 0x2a
              // STOP
              data: "0x60006000600060006000735fbdb2315678afecb367f032d93f642f64180aa361fffff1602a00",
              gas: "0x" + 1_000_000n.toString(16),
            },
          ],
        })
      );

      const rawTraces = responseObject.traces();
      assert.lengthOf(rawTraces, 1);

      const trace = rawTraces[0];
      const steps = collectSteps(trace);

      assert.lengthOf(steps, 13);
      assert.deepEqual(
        steps.map((step) => step.depth),
        [0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 0, 0]
      );

      assert.deepEqual(
        steps.map((step) => step.stack),
        [
          // Caller frame: PUSH1 0 (x5), PUSH20, PUSH2, CALL
          [],
          [0n],
          [0n],
          [0n],
          [0n],
          [0n],
          [calleeAddress],
          [0xffffn],
          // Callee frame: PUSH1 0x0a, PUSH1 0x0b, STOP
          [],
          [0x0an],
          [0x0bn],
          // Caller frame: PUSH1 0x2a sees the CALL success flag, then STOP
          [1n],
          [0x2an],
        ]
      );
    });

    it("should not include memory by default", async function () {
      const provider = await context.createProvider(
        GENERIC_CHAIN_TYPE,
        tracingProviderConfig,
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
              gas: "0x" + 1_000_000n.toString(16),
            },
          ],
        })
      );

      const rawTraces = responseObject.traces();
      assert.lengthOf(rawTraces, 1);

      const trace = rawTraces[0];
      const steps = collectSteps(trace);

      assert.lengthOf(steps, 4);

      // verbose tracing is disabled, so none of the steps should have memory
      assert.isTrue(steps.every((step) => step.memory === undefined));
    });

    it("should include memory if verbose mode is enabled", async function () {
      const provider = await context.createProvider(
        GENERIC_CHAIN_TYPE,
        tracingProviderConfig,
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
              gas: "0x" + 1_000_000n.toString(16),
            },
          ],
        })
      );

      const rawTraces = responseObject.traces();
      assert.lengthOf(rawTraces, 1);

      const trace = rawTraces[0];
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
        tracingProviderConfig,
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

      const rawTraces = responseObject.traces();
      assert.lengthOf(rawTraces, 1);

      const trace = rawTraces[0];
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
        tracingProviderConfig,
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

      const rawTraces = traceTransactionResponse.traces();
      assert.lengthOf(rawTraces, 1);
    });

    it("should have tracing information when debug_traceCall is used", async function () {
      const provider = await context.createProvider(
        GENERIC_CHAIN_TYPE,
        tracingProviderConfig,
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

      const rawTraces = traceCallResponse.traces();
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

  describe("setCallOverrideCallback", () => {
    it("invokes the callback and uses its return value for eth_call", async function () {
      const provider = await createGenericProvider(loggerConfig);

      let received: { addressLen: number; dataLen: number } | undefined;

      const RESULT = [0xca, 0xfe, 0xba, 0xbe];
      await provider.setCallOverrideCallback(
        (
          contractAddress: ArrayBuffer,
          data: ArrayBuffer
        ): Promise<CallOverrideResult | undefined> => {
          // Runtime value is a Uint8Array despite the ArrayBuffer annotation.
          received = {
            addressLen: Buffer.from(contractAddress).length,
            dataLen: Buffer.from(data).length,
          };
          return Promise.resolve({
            result: new Uint8Array(RESULT),
            shouldRevert: false,
          });
        }
      );

      const response = await provider.handleRequest(
        JSON.stringify({
          id: 1,
          jsonrpc: "2.0",
          method: "eth_call",
          params: [
            {
              to: "0xabababababababababababababababababababab",
              data: "0xdeadbeef",
              gas: GAS_BELOW_OSAKA_CAP,
            },
            "latest",
          ],
        })
      );

      assert.deepEqual(received, { addressLen: 20, dataLen: 4 });
      assert.equal(
        JSON.parse(response.data).result,
        "0x" + RESULT.map((byte) => byte.toString(16).padStart(2, "0")).join("")
      );
    });
  });

  describe("decodeConsoleLogInputsCallback", () => {
    it("surfaces a throwing callback as an error instead of crashing", async function () {
      const ERROR_MESSAGE = "decode exploded";
      const provider = await createGenericProvider({
        ...loggerConfig,
        decodeConsoleLogInputsCallback: (_inputs: ArrayBuffer[]): string[] => {
          throw new Error(ERROR_MESSAGE);
        },
      });

      const responseData = await sendConsoleLogHello(provider);

      assert.isDefined(responseData.error);
      assert.match(
        responseData.error.message,
        new RegExp(`Failed to decode console\\.log inputs.*${ERROR_MESSAGE}`)
      );
    });
  });

  describe("printLineCallback", () => {
    it("surfaces a throwing callback as an error instead of crashing", async function () {
      const ERROR_MESSAGE = "print exploded";
      const provider = await createGenericProvider({
        ...loggerConfig,
        decodeConsoleLogInputsCallback: (inputs: ArrayBuffer[]): string[] =>
          inputs.map(() => "hello"),
        printLineCallback: (_message: string, _replace: boolean) => {
          throw new Error(ERROR_MESSAGE);
        },
      });

      const responseData = await sendConsoleLogHello(provider);

      assert.isDefined(responseData.error);
      assert.match(
        responseData.error.message,
        new RegExp(`Failed to print line.*${ERROR_MESSAGE}`)
      );
    });
  });

  describe("subscriptionCallback", () => {
    it("delivers a SubscriptionEvent for each new block under a newHeads subscription", async function () {
      const events: SubscriptionEvent[] = [];
      let resolveFirst!: () => void;
      const firstEvent = new Promise<void>((resolve) => {
        resolveFirst = resolve;
      });

      const provider = await createGenericProvider(loggerConfig, (evt) => {
        events.push(evt);
        resolveFirst();
      });

      const subscribeResponse = await provider.handleRequest(
        JSON.stringify({
          id: 1,
          jsonrpc: "2.0",
          method: "eth_subscribe",
          params: ["newHeads"],
        })
      );
      const filterId = BigInt(JSON.parse(subscribeResponse.data).result);

      await provider.handleRequest(
        JSON.stringify({
          id: 2,
          jsonrpc: "2.0",
          method: "evm_mine",
          params: [],
        })
      );

      await firstEvent;

      assert.equal(events.length, 1);
      const event = events[0];
      assert.equal(typeof event.filterId, "bigint");
      assert.equal(event.filterId, filterId);

      // newHeads result is a block header; pin one well-known field rather
      // than the full structure to avoid coupling to RPC formatting details.
      function assertHasNumber(x: unknown): asserts x is { number: unknown } {
        if (typeof x !== "object" || x === null || !("number" in x)) {
          throw new Error("missing `number` field");
        }
      }

      assertHasNumber(event.result);
      assert.equal(typeof event.result.number, "string");
    });
  });

  describe("transactionGasCap", () => {
    // EIP-7825 caps transaction gas at MAX_TX_GAS_LIMIT_OSAKA = 16,777,216 on Osaka.
    const OSAKA_TRANSACTION_GAS_CAP = 16_777_216n;

    async function createProviderWithGasCap(
      transactionGasCap: bigint | false | undefined
    ): Promise<Provider> {
      return context.createProvider(
        GENERIC_CHAIN_TYPE,
        {
          ...providerConfig,
          hardfork: l1HardforkToString(SpecId.Osaka),
          genesisState: providerConfig.genesisState.concat(
            l1GenesisState(SpecId.Osaka)
          ),
          transactionGasCap,
        },
        loggerConfig,
        {
          subscriptionCallback: (_event: SubscriptionEvent) => {},
        },
        new ContractDecoder()
      );
    }

    async function sendTransactionWithGas(
      provider: Provider,
      gas: bigint
    ): Promise<any> {
      const response = await provider.handleRequest(
        JSON.stringify({
          id: 1,
          jsonrpc: "2.0",
          method: "eth_sendTransaction",
          params: [
            {
              from: genesisAddress,
              to: genesisAddress,
              gas: "0x" + gas.toString(16),
            },
          ],
        })
      );
      return JSON.parse(response.data);
    }

    it("uses the EIP-7825 cap on Osaka by default", async function () {
      const provider = await createProviderWithGasCap(undefined);

      const exceedsOsakaCap = OSAKA_TRANSACTION_GAS_CAP + 1n;
      const responseData = await sendTransactionWithGas(
        provider,
        exceedsOsakaCap
      );

      assert.isDefined(responseData.error);
      assert.include(
        responseData.error.message,
        `exceeds transaction gas cap of ${OSAKA_TRANSACTION_GAS_CAP}`
      );
    });

    it("accepts transactions at the default Osaka cap", async function () {
      const provider = await createProviderWithGasCap(undefined);

      const responseData = await sendTransactionWithGas(
        provider,
        OSAKA_TRANSACTION_GAS_CAP
      );

      assert.isUndefined(responseData.error);
      assert.isString(responseData.result);
    });

    it("enforces a custom numeric cap", async function () {
      const customCap = 50_000n;
      const provider = await createProviderWithGasCap(customCap);

      const exceedsCustomCap = customCap + 1n;
      const responseData = await sendTransactionWithGas(
        provider,
        exceedsCustomCap
      );

      assert.isDefined(responseData.error);
      assert.include(
        responseData.error.message,
        `exceeds transaction gas cap of ${customCap}`
      );
    });

    it("accepts transactions that exceed the default Osaka cap when set to `false`", async function () {
      const provider = await createProviderWithGasCap(false);

      // 20M is above the default Osaka cap (~16.7M) but below the test block
      // gas limit (300M).
      const exceedsOsakaCap = 20_000_000n;
      const responseData = await sendTransactionWithGas(
        provider,
        exceedsOsakaCap
      );

      assert.isUndefined(responseData.error);
      assert.isString(responseData.result);
    });

    it("rejects `true` as an invalid value", async function () {
      // The TS type forbids `true`; cast to bypass for the runtime check.
      await assert.isRejected(
        createProviderWithGasCap(true as unknown as false),
        /Boolean value for `transactionGasCap` must be false to disable the transaction gas cap/
      );
    });
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
          network: {
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
