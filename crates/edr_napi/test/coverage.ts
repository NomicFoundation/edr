import { toBytes } from "@nomicfoundation/ethereumjs-util";
import chai, { assert, expect } from "chai";
import chaiAsPromised from "chai-as-promised";
import * as fs from "fs";

import {
  AccountOverride,
  ContractDecoder,
  GENERIC_CHAIN_TYPE,
  genericChainProviderFactory,
  L1_CHAIN_TYPE,
  l1GenesisState,
  l1HardforkFromString,
  l1HardforkLatest,
  l1HardforkToString,
  l1SolidityTestRunnerFactory,
  MineOrdering,
  SubscriptionEvent,
  TestStatus,
} from "..";
import { getContext, loadContract, runAllSolidityTests } from "./helpers";

chai.use(chaiAsPromised);

class CoverageReporter {
  public hits: Uint8Array[] = [];
}

function readDeployedBytecode(): string {
  const filePath = `${__dirname}/../../../data/deployed_bytecode/increment.in`;
  return fs.readFileSync(filePath, "utf8");
}

describe("Code coverage", () => {
  const context = getContext();

  const incrementDeployedBytecode = readDeployedBytecode();

  // > cast calldata 'function incBy(uint)' 1
  const incrementCallData =
    "0x70119d060000000000000000000000000000000000000000000000000000000000000001";

  const ERROR_MESSAGE = "Simulated error in callback";

  let coverageReporter: CoverageReporter;
  before(async () => {
    await context.registerProviderFactory(
      GENERIC_CHAIN_TYPE,
      genericChainProviderFactory()
    );

    await context.registerSolidityTestRunnerFactory(
      L1_CHAIN_TYPE,
      l1SolidityTestRunnerFactory()
    );
  });

  beforeEach(() => {
    // Reset the coverage reporter
    coverageReporter = new CoverageReporter();
  });

  const genesisState: AccountOverride[] = [
    {
      address: toBytes("0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"),
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
    observability: {
      codeCoverage: {
        onCollectedCoverageCallback: async (coverage: Uint8Array[]) => {
          coverageReporter.hits.push(...coverage);
        },
      },
    },
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

  describe("eth_sendTransaction", function () {
    it("should report code coverage hits", async function () {
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

      const sendTransactionResponse = await provider.handleRequest(
        JSON.stringify({
          id: 1,
          jsonrpc: "2.0",
          method: "eth_sendTransaction",
          params: [
            {
              from: "0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266",
              data: incrementDeployedBytecode,
            },
          ],
        })
      );

      const transactionHash = JSON.parse(sendTransactionResponse.data).result;

      const transactionReceiptResponse = await provider.handleRequest(
        JSON.stringify({
          id: 1,
          jsonrpc: "2.0",
          method: "eth_getTransactionReceipt",
          params: [transactionHash],
        })
      );

      const deployedAddress = JSON.parse(transactionReceiptResponse.data).result
        .contractAddress;

      const _responseObject = await provider.handleRequest(
        JSON.stringify({
          id: 1,
          jsonrpc: "2.0",
          method: "eth_sendTransaction",
          params: [
            {
              from: "0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266",
              to: deployedAddress,
              data: incrementCallData,
            },
          ],
        })
      );

      assert.lengthOf(coverageReporter.hits, 2);
      expect(coverageReporter.hits).to.deep.include.members([
        Buffer.from(
          "0000000000000000000000000000000000000000000000000000000000000001",
          "hex"
        ),
        Buffer.from(
          "0000000000000000000000000000000000000000000000000000000000000002",
          "hex"
        ),
      ]);
    });

    it("should handle thrown exception", async function () {
      const provider = await context.createProvider(
        GENERIC_CHAIN_TYPE,
        {
          ...providerConfig,
          genesisState: providerConfig.genesisState.concat(
            l1GenesisState(l1HardforkFromString(providerConfig.hardfork))
          ),
          observability: {
            codeCoverage: {
              onCollectedCoverageCallback: async (_coverage: Uint8Array[]) => {
                throw new Error(ERROR_MESSAGE);
              },
            },
          },
        },
        loggerConfig,
        {
          subscriptionCallback: (_event: SubscriptionEvent) => {},
        },
        new ContractDecoder()
      );

      const sendTransactionResponse = await provider.handleRequest(
        JSON.stringify({
          id: 1,
          jsonrpc: "2.0",
          method: "eth_sendTransaction",
          params: [
            {
              from: "0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266",
              data: incrementDeployedBytecode,
            },
          ],
        })
      );

      const error = JSON.parse(sendTransactionResponse.data).error;

      assert.equal(error.code, -32603);
      assert(
        error.message.includes(ERROR_MESSAGE),
        "Error message should contain the expected error"
      );
    });
  });

  describe("Solidity test runner", function () {
    it("should report code coverage hits", async function () {
      const artifacts = [
        loadContract(
          "./data/artifacts/instrumented/SetupConsistencyCheck.json"
        ),
        loadContract("./data/artifacts/instrumented/PaymentFailureTest.json"),
      ];
      // All artifacts are test suites.
      const testSuites = artifacts.map((artifact) => artifact.id);
      const config = {
        projectRoot: __dirname,
        observability: {
          codeCoverage: {
            onCollectedCoverageCallback: async (coverage: Uint8Array[]) => {
              coverageReporter.hits.push(...coverage);
            },
          },
        },
      };

      await runAllSolidityTests(
        context,
        L1_CHAIN_TYPE,
        artifacts,
        testSuites,
        config
      );

      assert(coverageReporter.hits.length > 0, "No coverage hits reported");
    });

    it("should handle thrown exception", async function () {
      const artifacts = [
        loadContract(
          "./data/artifacts/instrumented/SetupConsistencyCheck.json"
        ),
        loadContract("./data/artifacts/instrumented/PaymentFailureTest.json"),
      ];
      // All artifacts are test suites.
      const testSuites = artifacts.map((artifact) => artifact.id);
      const config = {
        projectRoot: __dirname,
        observability: {
          codeCoverage: {
            onCollectedCoverageCallback: async (_coverage: Uint8Array[]) => {
              throw new Error(ERROR_MESSAGE);
            },
          },
        },
      };

      const suiteResults = await runAllSolidityTests(
        context,
        L1_CHAIN_TYPE,
        artifacts,
        testSuites,
        config
      );

      for (const suiteResult of suiteResults) {
        for (const testResult of suiteResult.testResults) {
          assert.equal(testResult.status, TestStatus.Failure);
          assert.isDefined(testResult.reason);
          assert(
            testResult.reason!.includes(ERROR_MESSAGE),
            `Test failure reason should contain the expected error. Found: ${testResult.reason}`
          );
        }
      }
    });
  });
});
