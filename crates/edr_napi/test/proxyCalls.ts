import { assert } from "chai";
import * as fs from "fs";
import {
  ContractDecoder,
  GENERIC_CHAIN_TYPE,
  Provider,
  SubscriptionEvent,
  TracingConfigWithBuffers,
} from "..";
import {
  DEFAULT_GENESIS_ADDRESS,
  deployContract,
  fundedGenesisState,
  getContext,
  l1ProviderConfig,
  registerGenericProviderFactory,
  sendTransaction,
} from "./helpers";

// Contract code in edr/data/contracts/ProxyMultipleImplementations.sol.
// The proxies delegate through their fallback functions, so their ABIs do not
// contain the implementations' functions. This makes calls through them hit
// the proxy chain resolution in `ContractDecoder::populate_call_trace_arena`.
const proxyBuildInfo: Buffer = fs.readFileSync(
  `${__dirname}/data/artifacts/default/ProxyMultipleImplementations.json`
);

const proxyContracts = JSON.parse(proxyBuildInfo.toString()).output.contracts[
  "project/contracts/ProxyMultipleImplementations.sol"
];

// Selector of `one()`, from `evm.methodIdentifiers` in the build info.
const ONE_CALLDATA = "0x901717d1";

const tracingConfig: TracingConfigWithBuffers = {
  buildInfos: [Uint8Array.from(proxyBuildInfo)],
};

/** ABI-encodes an address as a 32-byte constructor argument. */
function encodeAddressArg(address: string): string {
  return address.slice(2).toLowerCase().padStart(64, "0");
}

describe("Proxy call logging", function () {
  const context = getContext();

  before(async function () {
    await registerGenericProviderFactory(context);
  });

  let lines: string[];
  let provider: Provider;

  beforeEach(async function () {
    lines = [];

    provider = await context.createProvider(
      GENERIC_CHAIN_TYPE,
      l1ProviderConfig({ genesisState: fundedGenesisState() }),
      {
        enable: true,
        decodeConsoleLogInputsCallback: (
          _inputs: ArrayBuffer[]
        ): string[] => [],
        printLineCallback: (message: string, replace: boolean): void => {
          if (replace) {
            lines[lines.length - 1] = message;
          } else {
            lines.push(message);
          }
        },
      },
      {
        subscriptionCallback: (_event: SubscriptionEvent) => {},
      },
      ContractDecoder.withContracts(tracingConfig)
    );
  });

  function assertContractCallLine(expected: string) {
    const line = lines.find((entry) => entry.includes("Contract call:"));
    assert.isDefined(
      line,
      `Expected a "Contract call:" line in the logs:\n${lines.join("\n")}`
    );
    assert.match(
      line,
      new RegExp(`Contract call:\\s+${expected}$`),
      `Unexpected contract call line in the logs:\n${lines.join("\n")}`
    );
  }

  async function deployImpl1(): Promise<string> {
    return deployContract(
      provider,
      `0x${proxyContracts.Impl1.evm.bytecode.object}`,
      DEFAULT_GENESIS_ADDRESS
    );
  }

  async function deployProxy(
    contractName: "Proxy" | "Proxy2",
    implementationAddress: string
  ): Promise<string> {
    const bytecode: string =
      proxyContracts[contractName].evm.bytecode.object +
      encodeAddressArg(implementationAddress);

    return deployContract(provider, `0x${bytecode}`, DEFAULT_GENESIS_ADDRESS);
  }

  it("logs the function of a directly called contract", async function () {
    const implAddress = await deployImpl1();

    lines = [];
    await sendTransaction(provider, {
      from: DEFAULT_GENESIS_ADDRESS,
      to: implAddress,
      gas: 1_000_000,
      data: ONE_CALLDATA,
    });

    assertContractCallLine("Impl1#one");
  });

  it("logs the implementation's function for a call through a proxy", async function () {
    const implAddress = await deployImpl1();
    const proxyAddress = await deployProxy("Proxy", implAddress);

    lines = [];
    await sendTransaction(provider, {
      from: DEFAULT_GENESIS_ADDRESS,
      to: proxyAddress,
      gas: 1_000_000,
      data: ONE_CALLDATA,
    });

    assertContractCallLine("Proxy>Impl1#one");
    assert.isUndefined(
      lines.find((entry) => entry.includes("<unrecognized-selector>")),
      `Expected no unrecognized selector in the logs:\n${lines.join("\n")}`
    );
  });

  it("logs the full proxy chain for a call through chained proxies", async function () {
    const implAddress = await deployImpl1();
    const innerProxyAddress = await deployProxy("Proxy2", implAddress);
    const outerProxyAddress = await deployProxy("Proxy", innerProxyAddress);

    // `Proxy2`'s code reads its implementation from storage slot 1. Under
    // DELEGATECALL it reads the outer proxy's storage, so wire the outer
    // proxy's slot 1 to the implementation, mirroring the `vm.store` usage in
    // edr/js/integration-tests/solidity-tests/test-contracts/ProxyGasReport.t.sol.
    await provider.handleRequest(
      JSON.stringify({
        id: 1,
        jsonrpc: "2.0",
        method: "hardhat_setStorageAt",
        params: [
          outerProxyAddress,
          "0x1",
          `0x${encodeAddressArg(implAddress)}`,
        ],
      })
    );

    lines = [];
    await sendTransaction(provider, {
      from: DEFAULT_GENESIS_ADDRESS,
      to: outerProxyAddress,
      gas: 1_000_000,
      data: ONE_CALLDATA,
    });

    assertContractCallLine("Proxy>Proxy2>Impl1#one");
  });

  it("logs the implementation's function for an eth_call through a proxy", async function () {
    const implAddress = await deployImpl1();
    const proxyAddress = await deployProxy("Proxy", implAddress);

    lines = [];
    await provider.handleRequest(
      JSON.stringify({
        id: 1,
        jsonrpc: "2.0",
        method: "eth_call",
        params: [
          {
            from: DEFAULT_GENESIS_ADDRESS,
            to: proxyAddress,
            gas: "0xf4240", // 1,000,000
            data: ONE_CALLDATA,
          },
        ],
      })
    );

    assertContractCallLine("Proxy>Impl1#one");
  });

  it("falls back to an unrecognized selector for a proxied call to an unknown function", async function () {
    const implAddress = await deployImpl1();
    const proxyAddress = await deployProxy("Proxy", implAddress);

    lines = [];
    // A selector that is neither in the proxy's nor the implementation's ABI.
    await provider.handleRequest(
      JSON.stringify({
        id: 1,
        jsonrpc: "2.0",
        method: "eth_call",
        params: [
          {
            from: DEFAULT_GENESIS_ADDRESS,
            to: proxyAddress,
            gas: "0xf4240", // 1,000,000
            data: "0xdeadbeef",
          },
        ],
      })
    );

    assertContractCallLine("Proxy#<unrecognized-selector>");
  });
});
