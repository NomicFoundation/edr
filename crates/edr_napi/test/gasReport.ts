import { toBytes } from "@nomicfoundation/ethereumjs-util";
import { assert } from "chai";
import * as fs from "fs";
import {
  AccountOverride,
  ContractDecoder,
  GasReport,
  GasReportExecutionStatus,
  GENERIC_CHAIN_TYPE,
  genericChainProviderFactory,
  l1GenesisState,
  l1HardforkFromString,
  MineOrdering,
  Provider,
  SHANGHAI,
  SubscriptionEvent,
  TracingConfigWithBuffers,
} from "..";
import {
  deployContract,
  getContext,
  getGasPrice,
  sendTransaction,
} from "./helpers";

const genesisState: AccountOverride[] = [
  {
    address: toBytes("0xbe862ad9abfe6f22bcb087716c7d89a26051f74c"),
    balance: 1000n * 10n ** 18n,
  },
  {
    address: toBytes("0x94a48723b9b46b19c72e3091838d0522618b9363"),
    balance: 1000n * 10n ** 18n,
  },
];

class GasReporter {
  public report: GasReport | undefined;
}

// Contract build info in edr/crates/edr_napi/data/artifacts/default/GasReport.json
const contractBuildInfo: Buffer = fs.readFileSync(
  `${__dirname}/data/artifacts/default/GasReport.json`
);

// Contract code in edr/data/contracts/ProxyGasReport.sol
// Contract build info in edr/crates/edr_napi/data/artifacts/default/ProxyGasReport.json
const proxyBuildInfo: Buffer = fs.readFileSync(
  `${__dirname}/data/artifacts/default/ProxyGasReport.json`
);

const providerConfig = {
  allowBlocksWithSameTimestamp: false,
  allowUnlimitedContractSize: true,
  bailOnCallFailure: false,
  bailOnTransactionFailure: false,
  blockGasLimit: 6_000_000n,
  chainId: 123n,
  chainOverrides: [],
  coinbase: Uint8Array.from(
    Buffer.from("0000000000000000000000000000000000000000", "hex")
  ),
  genesisState,
  hardfork: SHANGHAI,
  initialBlobGas: {
    gasUsed: 0n,
    excessGas: 0n,
  },
  initialParentBeaconBlockRoot: Uint8Array.from(
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
    "0xe331b6d69882b4cb4ea581d88e0b604039a3de5967688d3dcffdd2270c0fd109",
    "0xe331b6d69882b4cb4ea581d88e0b604039a3de5967688d3dcffdd2270c0fd10a",
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

const tracingConfig: TracingConfigWithBuffers = {
  buildInfos: [Uint8Array.from(contractBuildInfo)],
  ignoreContracts: true,
};

describe("Gas reports", function () {
  const context = getContext();
  before(async () => {
    await context.registerProviderFactory(
      GENERIC_CHAIN_TYPE,
      genericChainProviderFactory()
    );
  });

  let provider: Provider;
  let gasPrice: bigint;
  let gasReporter: GasReporter;

  beforeEach(async function () {
    provider = await context.createProvider(
      GENERIC_CHAIN_TYPE,
      {
        ...providerConfig,
        genesisState: providerConfig.genesisState.concat(
          l1GenesisState(l1HardforkFromString(providerConfig.hardfork))
        ),
        observability: {
          gasReport: {
            onCollectedGasReportCallback: async (report: GasReport) => {
              gasReporter.report = report;
            },
          },
        },
      },
      loggerConfig,
      {
        subscriptionCallback: (_event: SubscriptionEvent) => {},
      },
      ContractDecoder.withContracts(tracingConfig)
    );

    gasPrice = await getGasPrice(provider);
    gasReporter = new GasReporter();
  });

  const exampleBuildInfo = JSON.parse(contractBuildInfo.toString());

  describe("sendTransaction", function () {
    it("deployment + transaction", async function () {
      const bytecode =
        exampleBuildInfo.output.contracts["project/contracts/MyLibrary.sol"]
          .MyLibrary.evm.bytecode.object;

      const address = await deployContract(provider, bytecode);

      assert.isDefined(
        gasReporter.report,
        "No gas report received after deployment"
      );

      let gasReport = gasReporter.report!;

      assert.equal(
        Object.keys(gasReport.contracts).length,
        1,
        "Gas report should contain exactly one contract"
      );
      assert.equal(
        Object.keys(gasReport.contracts)[0],
        "project/contracts/MyLibrary.sol:MyLibrary",
        "Gas report contains unexpected contract"
      );

      let contractReport =
        gasReport.contracts["project/contracts/MyLibrary.sol:MyLibrary"];

      assert.equal(
        contractReport.deployments.length,
        1,
        "Gas report should contain exactly one deployment"
      );
      assert.equal(
        Object.keys(contractReport.functions).length,
        0,
        "Gas report should contain no function calls after deployment"
      );

      const deployment = contractReport.deployments[0];
      assert.equal(
        deployment.gas,
        142_395n,
        "Gas report deployment has unexpected gas used"
      );

      const bytecodeLength = BigInt(bytecode.length / 2); // 2 hex chars per byte
      assert.equal(
        deployment.size,
        bytecodeLength,
        "Gas report deployment size mismatch"
      );

      assert.equal(
        deployment.status,
        GasReportExecutionStatus.Success,
        "Gas report deployment has non-success status"
      );

      await sendTransaction(provider, {
        to: address,
        gas: 1_000_000,
        data: "0x68ba353b0000000000000000000000000000000000000000000000000000000000000001", // plus100(1)
        value: 1,
        gasPrice,
      }).catch(() => {});

      assert.isDefined(
        gasReporter.report,
        "No gas report received after transaction"
      );

      gasReport = gasReporter.report!;

      assert.equal(
        Object.keys(gasReport.contracts).length,
        1,
        "Gas report should contain exactly one contract"
      );
      assert.equal(
        Object.keys(gasReport.contracts)[0],
        "project/contracts/MyLibrary.sol:MyLibrary",
        "Gas report contains unexpected contract"
      );

      contractReport =
        gasReport.contracts["project/contracts/MyLibrary.sol:MyLibrary"];

      assert.equal(
        contractReport.deployments.length,
        0,
        "Gas report should contain no deployments"
      );
      assert.equal(
        Object.keys(contractReport.functions).length,
        1,
        "Gas report should contain exactly one function"
      );

      const func = contractReport.functions["plus100(uint256)"];
      assert.equal(
        func.length,
        1,
        "Gas report should contain exactly one call to plus100(uint256)"
      );

      const call = func[0];
      assert(call.gas > 0n, "Gas report function call has zero gas used");

      assert.equal(
        call.gas,
        21_944n,
        "Gas report function call has unexpected gas used"
      );
      assert(
        call.status === GasReportExecutionStatus.Success,
        "Gas report call to plus100(uint256) has non-success status"
      );
    });
  });

  describe("call", function () {
    it("deployment + call", async function () {
      const bytecode =
        exampleBuildInfo.output.contracts["project/contracts/MyLibrary.sol"]
          .MyLibrary.evm.bytecode.object;

      const address = await deployContract(provider, bytecode);

      assert.isDefined(
        gasReporter.report,
        "No gas report received after deployment"
      );

      let gasReport = gasReporter.report!;

      assert.equal(
        Object.keys(gasReport.contracts).length,
        1,
        "Gas report should contain exactly one contract"
      );
      assert.equal(
        Object.keys(gasReport.contracts)[0],
        "project/contracts/MyLibrary.sol:MyLibrary",
        "Gas report contains unexpected contract"
      );

      let contractReport =
        gasReport.contracts["project/contracts/MyLibrary.sol:MyLibrary"];

      assert.equal(
        contractReport.deployments.length,
        1,
        "Gas report should contain exactly one deployment"
      );
      assert.equal(
        Object.keys(contractReport.functions).length,
        0,
        "Gas report should contain no function calls after deployment"
      );

      const deployment = contractReport.deployments[0];
      assert.equal(
        deployment.gas,
        142_395n,
        "Gas report deployment has unexpected gas used"
      );

      const bytecodeLength = BigInt(bytecode.length / 2); // 2 hex chars per byte
      assert.equal(
        deployment.size,
        bytecodeLength,
        "Gas report deployment size mismatch"
      );

      assert.equal(
        deployment.status,
        GasReportExecutionStatus.Success,
        "Gas report deployment has non-success status"
      );

      await provider
        .handleRequest(
          JSON.stringify({
            id: 1,
            jsonrpc: "2.0",
            method: "eth_call",
            params: [
              {
                to: address,
                data: "0x68ba353b0000000000000000000000000000000000000000000000000000000000000001", // plus100(1)
              },
            ],
          })
        )
        .catch(() => {});

      assert.isDefined(
        gasReporter.report,
        "No gas report received after transaction"
      );

      gasReport = gasReporter.report!;

      assert.equal(
        Object.keys(gasReport.contracts).length,
        1,
        "Gas report should contain exactly one contract"
      );
      assert.equal(
        Object.keys(gasReport.contracts)[0],
        "project/contracts/MyLibrary.sol:MyLibrary",
        "Gas report contains unexpected contract"
      );

      contractReport =
        gasReport.contracts["project/contracts/MyLibrary.sol:MyLibrary"];

      assert.equal(
        contractReport.deployments.length,
        0,
        "Gas report should contain no deployments"
      );
      assert.equal(
        Object.keys(contractReport.functions).length,
        1,
        "Gas report should contain exactly one function"
      );

      const func = contractReport.functions["plus100(uint256)"];
      assert.equal(
        func.length,
        1,
        "Gas report should contain exactly one call to plus100(uint256)"
      );

      const call = func[0];
      assert.equal(
        call.gas,
        21_944n,
        "Gas report function call has unexpected gas used"
      );

      assert(
        call.gas > 0,
        "Gas report call to plus100(uint256) has zero gas used"
      );
      assert(
        call.status === GasReportExecutionStatus.Success,
        "Gas report call to plus100(uint256) has non-success status"
      );
    });
  });

  describe("proxyChain", function () {
    const proxyTracingConfig: TracingConfigWithBuffers = {
      buildInfos: [Uint8Array.from(proxyBuildInfo)],
      ignoreContracts: false,
    };

    let proxyProvider: Provider;
    let proxyGasPrice: bigint;
    let proxyGasReporter: GasReporter;

    const proxyBuildInfoParsed = JSON.parse(proxyBuildInfo.toString());
    const proxyContracts =
      proxyBuildInfoParsed.output.contracts[
        "project/contracts/ProxyGasReport.sol"
      ];

    beforeEach(async function () {
      proxyGasReporter = new GasReporter();
      proxyProvider = await context.createProvider(
        GENERIC_CHAIN_TYPE,
        {
          ...providerConfig,
          genesisState: providerConfig.genesisState.concat(
            l1GenesisState(l1HardforkFromString(providerConfig.hardfork))
          ),
          observability: {
            gasReport: {
              onCollectedGasReportCallback: async (report: GasReport) => {
                proxyGasReporter.report = report;
              },
            },
          },
        },
        loggerConfig,
        {
          subscriptionCallback: (_event: SubscriptionEvent) => {},
        },
        ContractDecoder.withContracts(proxyTracingConfig)
      );

      proxyGasPrice = await getGasPrice(proxyProvider);
    });

    it("non-proxy call has empty proxyChain", async function () {
      const implBytecode = proxyContracts.Implementation.evm.bytecode.object;

      const implAddress = await deployContract(proxyProvider, implBytecode);

      // Call setValue(42) directly on Implementation
      // selector: 0x55241077
      const calldata =
        "0x55241077000000000000000000000000000000000000000000000000000000000000002a";

      await sendTransaction(proxyProvider, {
        to: implAddress,
        gas: 1_000_000,
        data: calldata,
        gasPrice: proxyGasPrice,
      });

      assert.isDefined(proxyGasReporter.report);
      const gasReport = proxyGasReporter.report!;

      const contractReport =
        gasReport.contracts[
          "project/contracts/ProxyGasReport.sol:Implementation"
        ];
      assert.isDefined(
        contractReport,
        "Gas report should contain Implementation contract"
      );

      const func = contractReport.functions["setValue(uint256)"];
      assert.isDefined(func, "Gas report should contain setValue function");
      assert.equal(func.length, 1);

      const call = func[0];
      assert.deepEqual(
        call.proxyChain,
        [],
        "Direct call should have empty proxyChain"
      );
    });

    it("proxy call has correct proxyChain", async function () {
      const implBytecode = proxyContracts.Implementation.evm.bytecode.object;

      const implAddress = await deployContract(proxyProvider, implBytecode);

      // Deploy Proxy with Implementation address as constructor arg
      const proxyBytecode: string = proxyContracts.Proxy.evm.bytecode.object;
      // ABI-encode constructor arg: address padded to 32 bytes
      const implAddrPadded = implAddress
        .slice(2)
        .toLowerCase()
        .padStart(64, "0");
      const proxyDeployCode = proxyBytecode + implAddrPadded;

      const proxyAddress = await deployContract(proxyProvider, proxyDeployCode);

      // Call setValue(42) through the Proxy
      // selector: 0x55241077
      const calldata =
        "0x55241077000000000000000000000000000000000000000000000000000000000000002a";

      await sendTransaction(proxyProvider, {
        to: proxyAddress,
        gas: 1_000_000,
        data: calldata,
        gasPrice: proxyGasPrice,
      });

      assert.isDefined(proxyGasReporter.report);
      const gasReport = proxyGasReporter.report!;

      // The gas report should attribute the call to the Proxy contract
      // (since that's the address we called)
      const contractReport =
        gasReport.contracts["project/contracts/ProxyGasReport.sol:Proxy"];
      assert.isDefined(
        contractReport,
        "Gas report should contain Proxy contract"
      );

      const func = contractReport.functions["setValue(uint256)"];
      assert.isDefined(
        func,
        "Gas report should contain setValue function on Proxy"
      );
      assert.equal(func.length, 1);

      const call = func[0];
      assert.equal(
        call.status,
        GasReportExecutionStatus.Success,
        "Proxy call should succeed"
      );
      assert(call.gas > 0n, "Proxy call should use gas");

      assert.equal(
        call.proxyChain.length,
        2,
        "Proxy call should have 2-entry proxyChain"
      );
      assert.equal(
        call.proxyChain[0],
        "project/contracts/ProxyGasReport.sol:Proxy",
        "First entry should be the Proxy"
      );
      assert.equal(
        call.proxyChain[1],
        "project/contracts/ProxyGasReport.sol:Implementation",
        "Second entry should be the Implementation"
      );
    });

    it("proxy call via eth_call has correct proxyChain", async function () {
      const implBytecode = proxyContracts.Implementation.evm.bytecode.object;

      const implAddress = await deployContract(proxyProvider, implBytecode);

      const proxyBytecode: string = proxyContracts.Proxy.evm.bytecode.object;
      const implAddrPadded = implAddress
        .slice(2)
        .toLowerCase()
        .padStart(64, "0");
      const proxyDeployCode = proxyBytecode + implAddrPadded;

      const proxyAddress = await deployContract(proxyProvider, proxyDeployCode);

      // Call setValue(42) through eth_call
      const calldata =
        "0x55241077000000000000000000000000000000000000000000000000000000000000002a";

      await proxyProvider.handleRequest(
        JSON.stringify({
          id: 1,
          jsonrpc: "2.0",
          method: "eth_call",
          params: [
            {
              to: proxyAddress,
              data: calldata,
            },
          ],
        })
      );

      assert.isDefined(proxyGasReporter.report);
      const gasReport = proxyGasReporter.report!;

      const contractReport =
        gasReport.contracts["project/contracts/ProxyGasReport.sol:Proxy"];
      assert.isDefined(
        contractReport,
        "Gas report should contain Proxy contract"
      );

      const func = contractReport.functions["setValue(uint256)"];
      assert.isDefined(
        func,
        "Gas report should contain setValue function on Proxy"
      );

      const call = func[0];
      assert.equal(
        call.proxyChain.length,
        2,
        "eth_call proxy should have 2-entry proxyChain"
      );
      assert.equal(
        call.proxyChain[0],
        "project/contracts/ProxyGasReport.sol:Proxy"
      );
      assert.equal(
        call.proxyChain[1],
        "project/contracts/ProxyGasReport.sol:Implementation"
      );
    });
  });
});
