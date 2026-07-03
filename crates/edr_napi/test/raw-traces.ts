import { toBytes } from "@nomicfoundation/ethereumjs-util";
import { assert } from "chai";
import {
  AccountOverride,
  CallKind,
  CallTrace,
  ContractDecoder,
  ExceptionalHalt,
  GENERIC_CHAIN_TYPE,
  genericChainProviderFactory,
  IncludeTraces,
  l1GenesisState,
  l1HardforkFromString,
  l1HardforkLatest,
  l1HardforkToString,
  LogKind,
  MineOrdering,
  Provider,
  Response,
  SubscriptionEvent,
  SuccessReason,
  TracingMessage,
  TracingMessageResult,
  TracingStep,
} from "..";
import { collectMessages, collectSteps, getContext } from "./helpers";

// Consumer-contract tests for the flat `Response.traces()` format that
// Hardhat 2's EthereumJS VM interface consumes. HH2-only surface: review for
// deletion at HH2 end-of-life.

const SENDER = "0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266";

// Runtime code: PUSH1 0x0a, PUSH1 0x0b, STOP
const CALLEE_INIT_CODE = "0x64600a600b005f526005601bf3";
// First contract deployed by SENDER
const CALLEE_ADDRESS = "0x5fbdb2315678afecb367f032d93f642f64180aa3";
// Init code: CALL to CALLEE_ADDRESS with 0xffff gas, then PUSH1 0x2a, STOP
const CALLER_INIT_CODE =
  "0x60006000600060006000735fbdb2315678afecb367f032d93f642f64180aa361fffff1602a00";
// Second contract deployed by SENDER
const CALLER_ADDRESS = "0xe7f1725e7734ce288f8367e1bb143e90bb3f0512";

const genesisState: AccountOverride[] = [
  {
    address: toBytes(SENDER),
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
  coinbase: Uint8Array.from(
    Buffer.from("0000000000000000000000000000000000000000", "hex")
  ),
  defaultTransactionGasLimit: 6_000_000n,
  genesisState,
  hardfork: l1HardforkToString(l1HardforkLatest()),
  initialParentBeaconBlockRoot: Uint8Array.from(
    Buffer.from(
      "0000000000000000000000000000000000000000000000000000000000000000",
      "hex"
    )
  ),
  minGasPrice: 0n,
  mining: {
    autoMine: true,
    blockGasLimit: 6_000_000n,
    memPool: {
      order: MineOrdering.Priority,
    },
  },
  network: {
    genesisBlobGas: {
      gasUsed: 0n,
      excessGas: 0n,
    },
    genesisBlockGasLimit: 6_000_000n,
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

type RawTraceItem = TracingMessage | TracingStep | TracingMessageResult;

// The discrimination pattern Hardhat 2's EdrProviderWrapper uses to dispatch
// beforeMessage/step/afterMessage events.
function itemKind(item: RawTraceItem): "message" | "step" | "result" {
  if ("pc" in item) {
    return "step";
  }
  if ("execResult" in item) {
    return "result";
  }
  return "message";
}

function collectResults(trace: RawTraceItem[]): TracingMessageResult[] {
  return trace.filter(
    (item): item is TracingMessageResult => "execResult" in item
  );
}

function addressBytes(address: string): Uint8Array {
  return Uint8Array.from(Buffer.from(address.slice(2), "hex"));
}

describe("Response.traces()", function () {
  const context = getContext();

  before(async () => {
    await context.registerProviderFactory(
      GENERIC_CHAIN_TYPE,
      genericChainProviderFactory()
    );
  });

  async function createProvider(
    includeCallTraces?: IncludeTraces
  ): Promise<Provider> {
    return context.createProvider(
      GENERIC_CHAIN_TYPE,
      {
        ...providerConfig,
        genesisState: providerConfig.genesisState.concat(
          l1GenesisState(l1HardforkFromString(providerConfig.hardfork))
        ),
        observability:
          includeCallTraces === undefined ? {} : { includeCallTraces },
      },
      loggerConfig,
      {
        subscriptionCallback: (_event: SubscriptionEvent) => {},
      },
      new ContractDecoder()
    );
  }

  async function sendTransactionResponse(
    provider: Provider,
    data: string
  ): Promise<Response> {
    return provider.handleRequest(
      JSON.stringify({
        id: 1,
        jsonrpc: "2.0",
        method: "eth_sendTransaction",
        params: [
          {
            from: SENDER,
            data,
            gas: "0x" + 1_000_000n.toString(16),
          },
        ],
      })
    );
  }

  async function sendNestedCallTransaction(
    provider: Provider
  ): Promise<Response> {
    await sendTransactionResponse(provider, CALLEE_INIT_CODE);
    return sendTransactionResponse(provider, CALLER_INIT_CODE);
  }

  it("emits message, steps, nested message/result, and result in EthereumJS order", async function () {
    const provider = await createProvider(IncludeTraces.All);
    const response = await sendNestedCallTransaction(provider);

    const rawTraces = response.traces();
    assert.lengthOf(rawTraces, 1);
    const trace = rawTraces[0];

    assert.deepEqual(trace.map(itemKind), [
      // Outer create frame: PUSH1 0 (x5), PUSH20, PUSH2, CALL
      "message",
      ...Array(8).fill("step"),
      // Inner call frame: PUSH1 0x0a, PUSH1 0x0b, STOP
      "message",
      ...Array(3).fill("step"),
      "result",
      // Outer frame resumes: PUSH1 0x2a, STOP
      ...Array(2).fill("step"),
      "result",
    ]);

    const [outerMessage, innerMessage] = collectMessages(trace);

    assert.deepEqual(outerMessage.caller, addressBytes(SENDER));
    assert.isUndefined(outerMessage.to);
    assert.isUndefined(outerMessage.codeAddress);
    assert.strictEqual(outerMessage.value, 0n);
    assert.deepEqual(
      outerMessage.data,
      Uint8Array.from(Buffer.from(CALLER_INIT_CODE.slice(2), "hex"))
    );
    assert.typeOf(outerMessage.gasLimit, "bigint");
    assert.isTrue(outerMessage.gasLimit > 0n);
    assert.isFalse(outerMessage.isStaticCall);

    assert.deepEqual(innerMessage.caller, addressBytes(CALLER_ADDRESS));
    assert.deepEqual(innerMessage.to, addressBytes(CALLEE_ADDRESS));
    assert.deepEqual(innerMessage.codeAddress, addressBytes(CALLEE_ADDRESS));
    assert.strictEqual(innerMessage.value, 0n);
    assert.deepEqual(innerMessage.data, new Uint8Array(0));
    assert.isFalse(innerMessage.isStaticCall);

    const [innerResult, outerResult] = collectResults(trace).map(
      (result) => result.execResult
    );

    assert.isTrue(innerResult.success);
    assert.strictEqual(innerResult.reason, SuccessReason.Stop);
    assert.isUndefined(innerResult.contractAddress);
    assert.deepEqual(innerResult.output, new Uint8Array(0));
    assert.typeOf(innerResult.executionGasUsed, "bigint");

    assert.isTrue(outerResult.success);
    // A successful create frame is normalized to Return, even though the init
    // code ended with STOP.
    assert.strictEqual(outerResult.reason, SuccessReason.Return);
    assert.deepEqual(outerResult.contractAddress, addressBytes(CALLER_ADDRESS));
    assert.deepEqual(outerResult.output, new Uint8Array(0));
  });

  it("reports a revert as success false without a reason", async function () {
    const provider = await createProvider(IncludeTraces.All);

    // Init code: MSTORE 0x42 as the first memory word, REVERT(0, 32)
    const response = await sendTransactionResponse(
      provider,
      "0x60425f5260205ffd"
    );

    const rawTraces = response.traces();
    assert.lengthOf(rawTraces, 1);
    const trace = rawTraces[0];

    assert.lengthOf(collectSteps(trace), 6);

    const [result] = collectResults(trace).map((item) => item.execResult);
    assert.isFalse(result.success);
    assert.isUndefined(result.reason);
    assert.isUndefined(result.contractAddress);
    assert.deepEqual(
      result.output,
      Uint8Array.from([...Array(31).fill(0), 0x42])
    );
  });

  it("reports an exceptional halt as success false with a reason", async function () {
    const provider = await createProvider(IncludeTraces.All);

    // Init code: INVALID
    const response = await sendTransactionResponse(provider, "0xfe");

    const rawTraces = response.traces();
    assert.lengthOf(rawTraces, 1);
    const trace = rawTraces[0];

    assert.lengthOf(collectMessages(trace), 1);

    const [result] = collectResults(trace).map((item) => item.execResult);
    assert.isFalse(result.success);
    assert.strictEqual(result.reason, ExceptionalHalt.InvalidFEOpcode);
    assert.isUndefined(result.contractAddress);
    assert.deepEqual(result.output, new Uint8Array(0));
  });

  it("returns an empty array when call traces are not enabled", async function () {
    const provider = await createProvider();
    const response = await sendNestedCallTransaction(provider);

    assert.deepEqual(response.traces(), []);
  });

  it("agrees with callTraces() on call structure for the same transaction", async function () {
    const provider = await createProvider(IncludeTraces.All);
    const response = await sendNestedCallTransaction(provider);

    const rawTraces = response.traces();
    const callTraces = response.callTraces();
    assert.lengthOf(rawTraces, 1);
    assert.lengthOf(callTraces, 1);

    const trace = rawTraces[0];
    const tree = callTraces[0];

    const messages = collectMessages(trace);
    const [innerResult, outerResult] = collectResults(trace).map(
      (result) => result.execResult
    );

    assert.strictEqual(tree.kind, CallKind.Create);
    assert.isUndefined(messages[0].to);

    const childCalls = tree.children.filter(
      (child): child is CallTrace => child.kind !== LogKind.Log
    );
    assert.lengthOf(childCalls, messages.length - 1);

    assert.strictEqual(
      tree.address.toLowerCase(),
      "0x" + Buffer.from(outerResult.contractAddress!).toString("hex")
    );
    assert.strictEqual(
      childCalls[0].address.toLowerCase(),
      "0x" + Buffer.from(messages[1].to!).toString("hex")
    );

    assert.strictEqual(tree.success, outerResult.success);
    assert.strictEqual(childCalls[0].success, innerResult.success);

    assert.strictEqual(tree.gasUsed, outerResult.executionGasUsed);
    assert.strictEqual(childCalls[0].gasUsed, innerResult.executionGasUsed);

    const stepDepths = collectSteps(trace).map((step) => step.depth);
    assert.strictEqual(Math.min(...stepDepths), 0);
    assert.strictEqual(Math.max(...stepDepths), 1);
  });
});
