import { toBytes } from "@nomicfoundation/ethereumjs-util";
import { assert } from "chai";
import {
  AccountOverride,
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
import { exampleBuildInfo } from "./helper/buildInfos";

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

const providerConfig = {
  allowBlocksWithSameTimestamp: false,
  allowUnlimitedContractSize: true,
  bailOnCallFailure: false,
  bailOnTransactionFailure: false,
  blockGasLimit: 6_000_000n,
  chainId: 123n,
  chainOverrides: [],
  coinbase: Buffer.from("0000000000000000000000000000000000000000", "hex"),
  genesisState,
  hardfork: SHANGHAI,
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
  buildInfos: [Buffer.from(JSON.stringify(exampleBuildInfo))],
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
      tracingConfig
    );

    gasPrice = await getGasPrice(provider);
    gasReporter = new GasReporter();
  });

  describe("sendTransaction", function () {
    it("deployment + transaction", async function () {
      const address = await deployContract(
        provider,
        exampleBuildInfo.output.contracts["contracts/Example.sol"].Example.evm
          .bytecode.object
      );

      assert.isDefined(gasReporter.report, "No gas report received");
      assert(
        gasReporter.report!.contracts["contracts/Example.sol:Example"]
          .deployments.length > 0,
        "Deployed contract not found in gas report"
      );
      assert(
        gasReporter.report!.contracts["contracts/Example.sol:Example"]
          .deployments[0].gas > 0,
        "Deployed contract has zero gas used in gas report"
      );
      assert(
        gasReporter.report!.contracts["contracts/Example.sol:Example"]
          .deployments[0].size > 0,
        "Deployed contract has zero size in gas report"
      );
      assert(
        gasReporter.report!.contracts["contracts/Example.sol:Example"]
          .deployments[0].status === GasReportExecutionStatus.Success,
        "Deployed contract has non-success status in gas report"
      );

      await sendTransaction(provider, {
        to: address,
        gas: 1000000,
        data: "0x0c55699c", // x()
        value: 1,
        gasPrice,
      }).catch(() => {});

      assert(
        Object.keys(
          gasReporter.report!.contracts["contracts/Example.sol:Example"]
            .functions
        ).length > 0,
        "No functions found in gas report"
      );
      assert(
        gasReporter.report!.contracts["contracts/Example.sol:Example"]
          .functions["x()"].calls.length > 0,
        "No calls to x() found in gas report"
      );

      assert(
        gasReporter.report!.contracts["contracts/Example.sol:Example"]
          .functions["x()"].calls[0].gas > 0,
        "Call to x() has zero gas used in gas report"
      );
      assert(
        gasReporter.report!.contracts["contracts/Example.sol:Example"]
          .functions["x()"].calls[0].status === GasReportExecutionStatus.Revert,
        "Call to x() has non-revert status in gas report"
      );
    });
  });

  describe("call", function () {
    it("deployment + call", async function () {
      const address = await deployContract(
        provider,
        exampleBuildInfo.output.contracts["contracts/Example.sol"].Example.evm
          .bytecode.object
      );

      assert.isDefined(gasReporter.report, "No gas report received");
      assert(
        gasReporter.report!.contracts["contracts/Example.sol:Example"]
          .deployments.length > 0,
        "Deployed contract not found in gas report"
      );
      assert(
        gasReporter.report!.contracts["contracts/Example.sol:Example"]
          .deployments[0].gas > 0,
        "Deployed contract has zero gas used in gas report"
      );
      assert(
        gasReporter.report!.contracts["contracts/Example.sol:Example"]
          .deployments[0].size > 0,
        "Deployed contract has zero size in gas report"
      );
      assert(
        gasReporter.report!.contracts["contracts/Example.sol:Example"]
          .deployments[0].status === GasReportExecutionStatus.Success,
        "Deployed contract has non-success status in gas report"
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
                data: "0x0c55699c", // x()
              },
            ],
          })
        )
        .catch(() => {});

      assert(
        Object.keys(
          gasReporter.report!.contracts["contracts/Example.sol:Example"]
            .functions
        ).length > 0,
        "No functions found in gas report"
      );
      assert(
        gasReporter.report!.contracts["contracts/Example.sol:Example"]
          .functions["x()"].calls.length > 0,
        "No calls to x() found in gas report"
      );

      assert(
        gasReporter.report!.contracts["contracts/Example.sol:Example"]
          .functions["x()"].calls[0].gas > 0,
        "Call to x() has zero gas used in gas report"
      );
      assert(
        gasReporter.report!.contracts["contracts/Example.sol:Example"]
          .functions["x()"].calls[0].status ===
          GasReportExecutionStatus.Success,
        "Call to x() has non-success status in gas report"
      );
    });
  });
});
