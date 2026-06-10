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
import { ALCHEMY_URL, getContext, loadContract } from "./helpers";

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

  // TODO(#1288): Add backwards compatibility for Hardhat 2
  // describe("verbose mode", function () {
  //   it("should only include the top of the stack by default", async function () {
  //     const provider = await context.createProvider(
  //       GENERIC_CHAIN_TYPE,
  //       {
  //         ...providerConfig,
  //         genesisState: providerConfig.genesisState.concat(
  //           l1GenesisState(l1HardforkFromString(providerConfig.hardfork))
  //         ),
  //       },
  //       loggerConfig,
  //       {
  //         subscriptionCallback: (_event: SubscriptionEvent) => {},
  //       },
  //       new ContractDecoder()
  //     );

  //     const responseObject = await provider.handleRequest(
  //       JSON.stringify({
  //         id: 1,
  //         jsonrpc: "2.0",
  //         method: "eth_sendTransaction",
  //         params: [
  //           {
  //             from: "0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266",
  //             // PUSH1 1
  //             // PUSH1 2
  //             // PUSH1 3
  //             // STOP
  //             data: "0x60016002600300",
  //           },
  //         ],
  //       })
  //     );

  //     const rawTraces = responseObject.traces;
  //     assert.lengthOf(rawTraces, 1);

  //     const trace = rawTraces[0].trace;
  //     const steps = collectSteps(trace);

  //     assert.lengthOf(steps, 4);

  //     assert.deepEqual(steps[0].stack, []);
  //     assert.deepEqual(steps[1].stack, [1n]);
  //     assert.deepEqual(steps[2].stack, [2n]);
  //     assert.deepEqual(steps[3].stack, [3n]);
  //   });

  //   it("should only include the whole stack if verbose mode is enabled", async function () {
  //     const provider = await context.createProvider(
  //       GENERIC_CHAIN_TYPE,
  //       {
  //         ...providerConfig,
  //         genesisState: providerConfig.genesisState.concat(
  //           l1GenesisState(l1HardforkFromString(providerConfig.hardfork))
  //         ),
  //       },
  //       loggerConfig,
  //       {
  //         subscriptionCallback: (_event: SubscriptionEvent) => {},
  //       },
  //       new ContractDecoder()
  //     );

  //     await provider.setVerboseTracing(true);

  //     const responseObject = await provider.handleRequest(
  //       JSON.stringify({
  //         id: 1,
  //         jsonrpc: "2.0",
  //         method: "eth_sendTransaction",
  //         params: [
  //           {
  //             from: "0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266",
  //             // PUSH1 1
  //             // PUSH1 2
  //             // PUSH1 3
  //             // STOP
  //             data: "0x60016002600300",
  //           },
  //         ],
  //       })
  //     );

  //     const rawTraces = responseObject.traces;
  //     assert.lengthOf(rawTraces, 1);

  //     const trace = rawTraces[0].trace;
  //     const steps = collectSteps(trace);

  //     assert.lengthOf(steps, 4);

  //     // verbose tracing is enabled, so all steps should have a stack
  //     assert.isTrue(steps.every((step) => step.stack !== undefined));

  //     assert.deepEqual(steps[0].stack, []);
  //     assert.deepEqual(steps[1].stack, [1n]);
  //     assert.deepEqual(steps[2].stack, [1n, 2n]);
  //     assert.deepEqual(steps[3].stack, [1n, 2n, 3n]);
  //   });

  //   it("should not include memory by default", async function () {
  //     const provider = await context.createProvider(
  //       GENERIC_CHAIN_TYPE,
  //       {
  //         ...providerConfig,
  //         genesisState: providerConfig.genesisState.concat(
  //           l1GenesisState(l1HardforkFromString(providerConfig.hardfork))
  //         ),
  //       },
  //       loggerConfig,
  //       {
  //         subscriptionCallback: (_event: SubscriptionEvent) => {},
  //       },
  //       new ContractDecoder()
  //     );

  //     const responseObject = await provider.handleRequest(
  //       JSON.stringify({
  //         id: 1,
  //         jsonrpc: "2.0",
  //         method: "eth_sendTransaction",
  //         params: [
  //           {
  //             from: "0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266",
  //             // store 0x000...001 as the first memory word
  //             // PUSH1 1
  //             // PUSH0
  //             // MSTORE
  //             // STOP
  //             data: "0x60015f5200",
  //           },
  //         ],
  //       })
  //     );

  //     const rawTraces = responseObject.traces;
  //     assert.lengthOf(rawTraces, 1);

  //     const trace = rawTraces[0].trace;
  //     const steps = collectSteps(trace);

  //     assert.lengthOf(steps, 4);

  //     // verbose tracing is disabled, so none of the steps should have a stack
  //     assert.isTrue(steps.every((step) => step.memory === undefined));
  //   });

  //   it("should include memory if verbose mode is enabled", async function () {
  //     const provider = await context.createProvider(
  //       GENERIC_CHAIN_TYPE,
  //       {
  //         ...providerConfig,
  //         genesisState: providerConfig.genesisState.concat(
  //           l1GenesisState(l1HardforkFromString(providerConfig.hardfork))
  //         ),
  //       },
  //       loggerConfig,
  //       {
  //         subscriptionCallback: (_event: SubscriptionEvent) => {},
  //       },
  //       new ContractDecoder()
  //     );

  //     await provider.setVerboseTracing(true);

  //     const responseObject = await provider.handleRequest(
  //       JSON.stringify({
  //         id: 1,
  //         jsonrpc: "2.0",
  //         method: "eth_sendTransaction",
  //         params: [
  //           {
  //             from: "0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266",
  //             // store 0x000...001 as the first memory word
  //             // PUSH1 1
  //             // PUSH0
  //             // MSTORE
  //             // STOP
  //             data: "0x60015f5200",
  //           },
  //         ],
  //       })
  //     );

  //     const rawTraces = responseObject.traces;
  //     assert.lengthOf(rawTraces, 1);

  //     const trace = rawTraces[0].trace;
  //     const steps = collectSteps(trace);

  //     assert.lengthOf(steps, 4);

  //     assertEqualMemory(steps[0].memory, Uint8Array.from([]));
  //     assertEqualMemory(steps[1].memory, Uint8Array.from([]));
  //     assertEqualMemory(steps[2].memory, Uint8Array.from([]));
  //     assertEqualMemory(
  //       steps[3].memory,
  //       Uint8Array.from([...Array(31).fill(0), 1])
  //     );
  //   });

  //   it("should include isStaticCall flag in tracing messages", async function () {
  //     const provider = await context.createProvider(
  //       GENERIC_CHAIN_TYPE,
  //       {
  //         ...providerConfig,
  //         genesisState: providerConfig.genesisState.concat(
  //           l1GenesisState(l1HardforkFromString(providerConfig.hardfork))
  //         ),
  //       },
  //       loggerConfig,
  //       {
  //         subscriptionCallback: (_event: SubscriptionEvent) => {},
  //       },
  //       new ContractDecoder()
  //     );

  //     const responseObject = await provider.handleRequest(
  //       JSON.stringify({
  //         id: 1,
  //         jsonrpc: "2.0",
  //         method: "eth_sendTransaction",
  //         params: [
  //           {
  //             from: "0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266",
  //             // make a static call to the zero address
  //             // yul: staticcall(gas(), 0, 0, 0, 0, 0)
  //             data: "0x6000808080805afa00",
  //             gas: "0x" + 1_000_000n.toString(16),
  //           },
  //         ],
  //       })
  //     );

  //     const rawTraces = responseObject.traces;
  //     assert.lengthOf(rawTraces, 1);

  //     const trace = rawTraces[0].trace;
  //     const messageResults = collectMessages(trace);
  //     assert.lengthOf(messageResults, 2);

  //     // outer message
  //     assert.isFalse(messageResults[0].isStaticCall);

  //     // inner message triggered by STATICCALL
  //     assert.isTrue(messageResults[1].isStaticCall);
  //   });

  //   it("should have tracing information when debug_traceTransaction is used", async function () {
  //     const provider = await context.createProvider(
  //       GENERIC_CHAIN_TYPE,
  //       {
  //         ...providerConfig,
  //         genesisState: providerConfig.genesisState.concat(
  //           l1GenesisState(l1HardforkFromString(providerConfig.hardfork))
  //         ),
  //       },
  //       loggerConfig,
  //       {
  //         subscriptionCallback: (_event: SubscriptionEvent) => {},
  //       },
  //       new ContractDecoder()
  //     );

  //     const sendTxResponse = await provider.handleRequest(
  //       JSON.stringify({
  //         id: 1,
  //         jsonrpc: "2.0",
  //         method: "eth_sendTransaction",
  //         params: [
  //           {
  //             from: "0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266",
  //             // PUSH1 0x42
  //             // PUSH0
  //             // MSTORE
  //             // PUSH1 0x20
  //             // PUSH0
  //             // RETURN
  //             data: "0x60425f5260205ff3",
  //             gas: "0x" + 1_000_000n.toString(16),
  //           },
  //         ],
  //       })
  //     );

  //     let responseData;

  //     if (typeof sendTxResponse.data === "string") {
  //       responseData = JSON.parse(sendTxResponse.data);
  //     } else {
  //       responseData = sendTxResponse.data;
  //     }

  //     const txHash = responseData.result;

  //     const traceTransactionResponse = await provider.handleRequest(
  //       JSON.stringify({
  //         id: 1,
  //         jsonrpc: "2.0",
  //         method: "debug_traceTransaction",
  //         params: [txHash],
  //       })
  //     );

  //     const rawTraces = traceTransactionResponse.traces;
  //     assert.lengthOf(rawTraces, 1);
  //   });

  //   it("should have tracing information when debug_traceCall is used", async function () {
  //     const provider = await context.createProvider(
  //       GENERIC_CHAIN_TYPE,
  //       {
  //         ...providerConfig,
  //         genesisState: providerConfig.genesisState.concat(
  //           l1GenesisState(l1HardforkFromString(providerConfig.hardfork))
  //         ),
  //       },
  //       loggerConfig,
  //       {
  //         subscriptionCallback: (_event: SubscriptionEvent) => {},
  //       },
  //       new ContractDecoder()
  //     );

  //     const traceCallResponse = await provider.handleRequest(
  //       JSON.stringify({
  //         id: 1,
  //         jsonrpc: "2.0",
  //         method: "debug_traceCall",
  //         params: [
  //           {
  //             from: "0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266",
  //             // PUSH1 0x42
  //             // PUSH0
  //             // MSTORE
  //             // PUSH1 0x20
  //             // PUSH0
  //             // RETURN
  //             data: "0x60425f5260205ff3",
  //             gas: "0x" + 1_000_000n.toString(16),
  //           },
  //         ],
  //       })
  //     );

  //     const rawTraces = traceCallResponse.traces;
  //     assert.lengthOf(rawTraces, 1);
  //   });
  // });

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

      const targetAddress = "0xabababababababababababababababababababab";
      const callData = "0xdeadbeef";
      let received: { addressLen: number; dataLen: number } | undefined;

      await provider.setCallOverrideCallback(
        async (
          contractAddress: ArrayBuffer,
          data: ArrayBuffer
        ): Promise<CallOverrideResult | undefined> => {
          // index.d.ts annotates these as `ArrayBuffer` for HH2 backwards-compat
          // (HH2's provider.ts types its callback the same way and calls
          // `Buffer.from(x)` on the args). The actual runtime value under
          // napi-rs v3 is a `Uint8Array`; `Buffer.from(x)` accepts both shapes,
          // which is what makes the type/runtime skew safe. See the longer
          // note on `Provider::set_call_override_callback` in
          // `src/provider.rs` for why we can't produce a real `ArrayBuffer`
          // Rust-side under v3.
          received = {
            addressLen: Buffer.from(contractAddress).length,
            dataLen: Buffer.from(data).length,
          };
          return {
            result: new Uint8Array([0xca, 0xfe, 0xba, 0xbe]),
            shouldRevert: false,
          };
        }
      );

      const response = await provider.handleRequest(
        JSON.stringify({
          id: 1,
          jsonrpc: "2.0",
          method: "eth_call",
          // Explicit `gas` below the EIP-7825 transaction gas cap (16,777,216
          // on Osaka); without it the call inherits
          // `defaultTransactionGasLimit` (300M) and is rejected before the
          // call override can fire.
          params: [
            { to: targetAddress, data: callData, gas: "0xf4240" },
            "latest",
          ],
        })
      );

      assert.deepEqual(received, { addressLen: 20, dataLen: 4 });
      assert.equal(JSON.parse(response.data).result, "0xcafebabe");
    });
  });

  describe("decodeConsoleLogInputsCallback", () => {
    it("invokes the callback with Buffer.from-compatible args for console.log calls", async function () {
      const receivedInputLengths: number[] = [];
      const printedMessages: string[] = [];

      const provider = await context.createProvider(
        GENERIC_CHAIN_TYPE,
        {
          ...providerConfig,
          genesisState: providerConfig.genesisState.concat(
            l1GenesisState(l1HardforkFromString(providerConfig.hardfork))
          ),
        },
        {
          // With `enable: false` the decode callback still fires (see the
          // else-branch in `edr_napi_core/src/logger.rs::log_console_log_messages`)
          // and the decoded output is passed straight to `printLineCallback`
          // without the formatted "console.log:" header that `enable: true`
          // would add. Cleaner assertions than logs.ts's enable-true path.
          enable: false,
          decodeConsoleLogInputsCallback: (inputs: ArrayBuffer[]): string[] => {
            // index.d.ts annotates `inputs` as `ArrayBuffer[]` for HH2
            // backwards-compat (HH2's provider.ts:288 uses the same annotation
            // and calls `Buffer.from(input)`). The actual runtime value under
            // napi-rs v3 is `Uint8Array[]`; `Buffer.from(x)` accepts both. See
            // the longer note on `LoggerConfig::decode_console_log_inputs_callback`
            // in `src/logger.rs`.
            for (const input of inputs) {
              receivedInputLengths.push(Buffer.from(input).length);
            }
            // Hard-coded so the printedMessages assertion below is independent
            // of the `ConsoleLogger.getDecodedLogs` helper used in logs.ts.
            return inputs.map(() => "hello");
          },
          printLineCallback: (message: string, _replace: boolean) => {
            printedMessages.push(message);
          },
        },
        {
          subscriptionCallback: (_event: SubscriptionEvent) => {},
        },
        new ContractDecoder()
      );

      // ABI-encoded `console.log(string)` calldata for the message "hello":
      //   selector       0x41304fac          (4 bytes)
      //   string offset  0x20  (= 32)        (32 bytes)
      //   string length  0x05                (32 bytes)
      //   string data    0x68656c6c6f...     (32 bytes, right-padded)
      // Total: 4 + 96 = 100 bytes.
      const consoleLogHelloCalldata =
        "0x41304fac" +
        "0000000000000000000000000000000000000000000000000000000000000020" +
        "0000000000000000000000000000000000000000000000000000000000000005" +
        "68656c6c6f000000000000000000000000000000000000000000000000000000";

      // EDR's `ConsoleLogCollector` (`crates/edr_provider/src/console_log.rs`)
      // is an inspector that fires on any CALL frame whose bytecode_address is
      // `0x000000000000000000636f6e736f6c652e6c6f67` ("console.log"
      // right-padded). A top-level `eth_sendTransaction` to that address
      // triggers the hook without any contract deployment.
      const consoleLogAddress = "0x000000000000000000636f6e736f6c652e6c6f67";

      await provider.handleRequest(
        JSON.stringify({
          id: 1,
          jsonrpc: "2.0",
          method: "eth_sendTransaction",
          params: [
            {
              from: "0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266",
              to: consoleLogAddress,
              data: consoleLogHelloCalldata,
              // Explicit `gas` below the EIP-7825 transaction gas cap
              // (16,777,216 on Osaka); without it the transaction inherits
              // `defaultTransactionGasLimit` (300M) and is rejected before
              // execution, so the console.log inspector never fires.
              gas: "0xf4240",
            },
          ],
        })
      );

      // Asserts the decode callback fired and `Buffer.from()` accepted the
      // runtime value (the implicit assertion is "no exception thrown" — if
      // the runtime value were neither ArrayBuffer nor a TypedArray view,
      // `Buffer.from()` would throw a TypeError).
      assert.deepEqual(receivedInputLengths, [100]);
      // Asserts the decoded string reached `printLineCallback` end-to-end.
      assert.deepEqual(printedMessages, ["hello"]);
    });

    it("surfaces a throwing callback as an error instead of crashing", async function () {
      const provider = await context.createProvider(
        GENERIC_CHAIN_TYPE,
        {
          ...providerConfig,
          genesisState: providerConfig.genesisState.concat(
            l1GenesisState(l1HardforkFromString(providerConfig.hardfork))
          ),
        },
        {
          enable: false,
          decodeConsoleLogInputsCallback: (
            _inputs: ArrayBuffer[]
          ): string[] => {
            throw new Error("decode exploded");
          },
          printLineCallback: (_message: string, _replace: boolean) => {},
        },
        {
          subscriptionCallback: (_event: SubscriptionEvent) => {},
        },
        new ContractDecoder()
      );

      // Same console.log(string "hello") transaction as the test above.
      const consoleLogHelloCalldata =
        "0x41304fac" +
        "0000000000000000000000000000000000000000000000000000000000000020" +
        "0000000000000000000000000000000000000000000000000000000000000005" +
        "68656c6c6f000000000000000000000000000000000000000000000000000000";
      const consoleLogAddress = "0x000000000000000000636f6e736f6c652e6c6f67";

      // The JS exception must come back as a JSON-RPC error response, not
      // take the process down: before the napi-rs v3 error-propagation fix,
      // a throwing decode callback closed the result channel and panicked
      // the Rust side.
      const response = await provider.handleRequest(
        JSON.stringify({
          id: 1,
          jsonrpc: "2.0",
          method: "eth_sendTransaction",
          params: [
            {
              from: "0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266",
              to: consoleLogAddress,
              data: consoleLogHelloCalldata,
              gas: "0xf4240",
            },
          ],
        })
      );

      const responseData = JSON.parse(response.data);
      assert.isDefined(responseData.error);
      assert.match(
        responseData.error.message,
        /Failed to decode console\.log inputs.*decode exploded/
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
          subscriptionCallback: (evt: SubscriptionEvent) => {
            events.push(evt);
            resolveFirst();
          },
        },
        new ContractDecoder()
      );

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

      // Pins the SubscriptionEvent shape produced by the `compat-mode`
      // JsObject construction in `edr_napi_core/src/subscription.rs`. The
      // `compat-mode` feature is documented as temporary (Design decision §4
      // in the PR body); when it's removed in a follow-up, this test should
      // keep passing as long as the event shape stays the same.
      assert.equal(events.length, 1);
      const event = events[0];
      assert.equal(typeof event.filterId, "bigint");
      assert.equal(event.filterId, filterId);
      assert.notStrictEqual(event.result, null);
      assert.notStrictEqual(event.result, undefined);
      // newHeads result is a block header; pin one well-known field rather
      // than the full structure to avoid coupling to RPC formatting details.
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

// TODO(#1288): Add backwards compatibility for Hardhat 2
// function assertEqualMemory(
//   stepMemory: Uint8Array | undefined,
//   expected: Uint8Array
// ) {
//   if (stepMemory === undefined) {
//     assert.fail("step memory is undefined");
//   }

//   assert.deepEqual(stepMemory, expected);
// }
