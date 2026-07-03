import { toBytes } from "@nomicfoundation/ethereumjs-util";
import { assert } from "chai";
import * as fs from "fs";
import {
  AccountOverride,
  CallKind,
  CallTrace,
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
  Response,
  SubscriptionEvent,
  TracingConfigWithBuffers,
} from "..";
import { deployContract, getContext } from "./helpers";

const SENDER = "0xbe862ad9abfe6f22bcb087716c7d89a26051f74c";

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
    "0xe331b6d69882b4cb4ea581d88e0b604039a3de5967688d3dcffdd2270c0fd109",
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

// Contract code in edr/data/contracts/ProxyGasReport.sol
const proxyBuildInfo: Buffer = fs.readFileSync(
  `${__dirname}/data/artifacts/default/ProxyGasReport.json`
);

const tracingConfig: TracingConfigWithBuffers = {
  buildInfos: [Uint8Array.from(proxyBuildInfo)],
  ignoreContracts: false,
};

const proxyContracts = JSON.parse(proxyBuildInfo.toString()).output.contracts[
  "project/contracts/ProxyGasReport.sol"
];

// setValue(42)
const SET_VALUE_42_CALLDATA =
  "0x55241077000000000000000000000000000000000000000000000000000000000000002a";

describe("Response.callTraces()", function () {
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
      ContractDecoder.withContracts(tracingConfig)
    );
  }

  async function deployProxyAndImplementation(
    provider: Provider
  ): Promise<{ implAddress: string; proxyAddress: string }> {
    const implAddress = await deployContract(
      provider,
      proxyContracts.Implementation.evm.bytecode.object
    );

    const implAddressPadded = implAddress
      .slice(2)
      .toLowerCase()
      .padStart(64, "0");
    const proxyAddress = await deployContract(
      provider,
      proxyContracts.Proxy.evm.bytecode.object + implAddressPadded
    );

    return { implAddress, proxyAddress };
  }

  async function sendTransactionResponse(
    provider: Provider,
    to: string,
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
            to,
            data,
            gas: "0x" + 1_000_000n.toString(16),
          },
        ],
      })
    );
  }

  async function callResponse(
    provider: Provider,
    to: string,
    data: string
  ): Promise<Response> {
    return provider.handleRequest(
      JSON.stringify({
        id: 1,
        jsonrpc: "2.0",
        method: "eth_call",
        params: [
          {
            from: SENDER,
            to,
            data,
            gas: "0x" + 1_000_000n.toString(16),
          },
        ],
      })
    );
  }

  it("decodes a successful transaction into a call tree", async function () {
    const provider = await createProvider(IncludeTraces.All);
    const { implAddress, proxyAddress } =
      await deployProxyAndImplementation(provider);

    const response = await sendTransactionResponse(
      provider,
      proxyAddress,
      SET_VALUE_42_CALLDATA
    );

    const callTraces = response.callTraces();
    assert.lengthOf(callTraces, 1);

    const root = callTraces[0];
    assert.strictEqual(root.kind, CallKind.Call);
    assert.isTrue(root.success);
    assert.isFalse(root.isCheatcode);
    assert.typeOf(root.gasUsed, "bigint");
    assert.isTrue(root.gasUsed > 0n);
    assert.strictEqual(root.value, 0n);
    assert.strictEqual(root.address.toLowerCase(), proxyAddress.toLowerCase());
    assert.strictEqual(root.contract, "Proxy");
    assert.deepEqual(root.inputs, { name: "setValue", arguments: ["42"] });
    assert.strictEqual(root.outputs, "");

    assert.lengthOf(root.children, 1);
    const child = root.children[0] as CallTrace;
    assert.strictEqual(child.kind, CallKind.DelegateCall);
    assert.isTrue(child.success);
    assert.isFalse(child.isCheatcode);
    assert.typeOf(child.gasUsed, "bigint");
    assert.strictEqual(child.address.toLowerCase(), implAddress.toLowerCase());
    assert.strictEqual(child.contract, "Implementation");
    assert.deepEqual(child.inputs, { name: "setValue", arguments: ["42"] });
    assert.deepEqual(child.children, []);
  });

  it("includes a trace with success false for a reverted call", async function () {
    const provider = await createProvider(IncludeTraces.All);
    const { implAddress } = await deployProxyAndImplementation(provider);

    // Unknown selector on a contract without a fallback function
    const response = await callResponse(provider, implAddress, "0xdeadbeef");

    const callTraces = response.callTraces();
    assert.lengthOf(callTraces, 1);

    const root = callTraces[0];
    assert.strictEqual(root.kind, CallKind.Call);
    assert.isFalse(root.success);
    assert.strictEqual(root.contract, "Implementation");
    assert.deepEqual(root.inputs, {
      name: "<unrecognized-selector>",
      arguments: ["0xdeadbeef"],
    });
    assert.strictEqual(
      root.outputs,
      `unrecognized function selector 0xdeadbeef for contract Implementation (${root.address}).`
    );
  });

  it("represents an unknown contract creation with raw bytes", async function () {
    const provider = await createProvider(IncludeTraces.All);

    // Init code: PUSH1 1, PUSH1 2, PUSH1 3, STOP
    const initCode = "0x60016002600300";
    const response = await provider.handleRequest(
      JSON.stringify({
        id: 1,
        jsonrpc: "2.0",
        method: "eth_sendTransaction",
        params: [
          {
            from: SENDER,
            data: initCode,
            gas: "0x" + 1_000_000n.toString(16),
          },
        ],
      })
    );

    const callTraces = response.callTraces();
    assert.lengthOf(callTraces, 1);

    const root = callTraces[0];
    assert.strictEqual(root.kind, CallKind.Create);
    assert.isTrue(root.success);
    assert.strictEqual(root.contract, "<UnrecognizedContract>");
    assert.deepEqual(
      root.inputs,
      Uint8Array.from(Buffer.from(initCode.slice(2), "hex"))
    );
    assert.strictEqual(root.outputs, "0 bytes of code");
  });

  it("returns an empty array when call traces are not enabled", async function () {
    const provider = await createProvider();
    const { proxyAddress } = await deployProxyAndImplementation(provider);

    const response = await sendTransactionResponse(
      provider,
      proxyAddress,
      SET_VALUE_42_CALLDATA
    );

    assert.deepEqual(response.callTraces(), []);
  });

  it("with IncludeTraces.Failing, includes only failing executions", async function () {
    const provider = await createProvider(IncludeTraces.Failing);
    const { implAddress, proxyAddress } =
      await deployProxyAndImplementation(provider);

    const successResponse = await callResponse(
      provider,
      proxyAddress,
      SET_VALUE_42_CALLDATA
    );
    assert.deepEqual(successResponse.callTraces(), []);

    const failureResponse = await callResponse(
      provider,
      implAddress,
      "0xdeadbeef"
    );
    const callTraces = failureResponse.callTraces();
    assert.lengthOf(callTraces, 1);
    assert.isFalse(callTraces[0].success);
  });
});
