import { toBytes } from "@nomicfoundation/ethereumjs-util";
import { assert } from "chai";
import * as fs from "fs";
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
      tracingConfig
    );

    gasPrice = await getGasPrice(provider);
    gasReporter = new GasReporter();
  });

  const exampleBuildInfo = JSON.parse(contractBuildInfo.toString());

  describe("sendTransaction", function () {
    it("deployment + transaction", async function () {
      const address = await deployContract(
        provider,
        exampleBuildInfo.output.contracts["project/contracts/MyLibrary.sol"]
          .MyLibrary.evm.bytecode.object
      );

      let contractReport =
        gasReporter.report!.contracts[
          "project/contracts/MyLibrary.sol:MyLibrary"
        ];

      assert.isDefined(gasReporter.report, "No gas report received");
      assert(
        contractReport.deployments.length > 0,
        "Deployed contract not found in gas report"
      );
      assert(
        contractReport.deployments[0].gas > 0,
        "Deployed contract has zero gas used in gas report"
      );
      assert(
        contractReport.deployments[0].size > 0,
        "Deployed contract has zero size in gas report"
      );
      assert(
        contractReport.deployments[0].status ===
          GasReportExecutionStatus.Success,
        "Deployed contract has non-success status in gas report"
      );

      await sendTransaction(provider, {
        to: address,
        gas: 1000000,
        data: "0x68ba353b0000000000000000000000000000000000000000000000000000000000000001", // plus100(1)
        value: 1,
        gasPrice,
      }).catch(() => {});

      contractReport =
        gasReporter.report!.contracts[
          "project/contracts/MyLibrary.sol:MyLibrary"
        ];

      assert(
        Object.keys(contractReport.functions).length > 0,
        "No functions found in gas report"
      );
      assert(
        contractReport.functions["plus100(uint256)"].calls.length > 0,
        "No calls to plus100(uint256) found in gas report"
      );
      assert(
        contractReport.functions["plus100(uint256)"].calls[0].gas > 0,
        "Call to plus100(uint256) has zero gas used in gas report"
      );
      assert(
        contractReport.functions["plus100(uint256)"].calls[0].status ===
          GasReportExecutionStatus.Success,
        "Call to plus100(uint256) has non-success status in gas report"
      );
    });
  });

  describe("call", function () {
    it("deployment + call", async function () {
      const address = await deployContract(
        provider,
        exampleBuildInfo.output.contracts["project/contracts/MyLibrary.sol"]
          .MyLibrary.evm.bytecode.object
      );

      let contractReport =
        gasReporter.report!.contracts[
          "project/contracts/MyLibrary.sol:MyLibrary"
        ];

      assert.isDefined(gasReporter.report, "No gas report received");
      assert(
        contractReport.deployments.length > 0,
        "Deployed contract not found in gas report"
      );
      assert(
        contractReport.deployments[0].gas > 0,
        "Deployed contract has zero gas used in gas report"
      );
      assert(
        contractReport.deployments[0].size > 0,
        "Deployed contract has zero size in gas report"
      );
      assert(
        contractReport.deployments[0].status ===
          GasReportExecutionStatus.Success,
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
                data: "0x68ba353b0000000000000000000000000000000000000000000000000000000000000001", // plus100(1)
              },
            ],
          })
        )
        .catch(() => {});

      contractReport =
        gasReporter.report!.contracts[
          "project/contracts/MyLibrary.sol:MyLibrary"
        ];

      assert(
        Object.keys(contractReport.functions).length > 0,
        "No functions found in gas report"
      );
      assert(
        contractReport.functions["plus100(uint256)"].calls.length > 0,
        "No calls to plus100(uint256) found in gas report"
      );
      assert(
        contractReport.functions["plus100(uint256)"].calls[0].gas > 0,
        "Call to plus100(uint256) has zero gas used in gas report"
      );
      assert(
        contractReport.functions["plus100(uint256)"].calls[0].status ===
          GasReportExecutionStatus.Success,
        "Call to plus100(uint256) has non-success status in gas report"
      );
    });
  });
});
