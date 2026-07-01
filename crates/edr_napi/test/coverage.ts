import chai, { assert, expect } from "chai";
import chaiAsPromised from "chai-as-promised";
import * as fs from "fs";

import {
  L1_CHAIN_TYPE,
  l1HardforkLatest,
  l1HardforkToString,
  l1SolidityTestRunnerFactory,
  TestStatus,
} from "..";
import {
  createL1Provider,
  fundedGenesisState,
  getContext,
  loadContract,
  registerGenericProviderFactory,
  runAllSolidityTests,
} from "./helpers";

chai.use(chaiAsPromised);

class CoverageReporter {
  public hits: Uint8Array[] = [];
}

function readDeploymentBytecode(): string {
  const filePath = `${__dirname}/../../../data/deployment_bytecode/Increment.bin`;
  return fs.readFileSync(filePath, "utf8");
}

describe("Code coverage", () => {
  const context = getContext();

  const incrementDeploymentBytecode = readDeploymentBytecode();

  // > cast calldata 'function incBy(uint)' 1
  const incrementCallData =
    "0x70119d060000000000000000000000000000000000000000000000000000000000000001";

  const ERROR_MESSAGE = "Simulated error in callback";

  let coverageReporter: CoverageReporter;
  before(async () => {
    await registerGenericProviderFactory(context);

    await context.registerSolidityTestRunnerFactory(
      L1_CHAIN_TYPE,
      l1SolidityTestRunnerFactory()
    );
  });

  beforeEach(() => {
    // Reset the coverage reporter
    coverageReporter = new CoverageReporter();
  });

  const providerOverrides = {
    defaultTransactionGasLimit: 16_777_216n,
    genesisState: fundedGenesisState(),
  };

  describe("eth_sendTransaction", function () {
    it("should report code coverage hits", async function () {
      const provider = await createL1Provider(context, {
        ...providerOverrides,
        observability: {
          codeCoverage: {
            onCollectedCoverageCallback: async (coverage: Uint8Array[]) => {
              coverageReporter.hits.push(...coverage);
            },
          },
        },
      });

      const sendTransactionResponse = await provider.handleRequest(
        JSON.stringify({
          id: 1,
          jsonrpc: "2.0",
          method: "eth_sendTransaction",
          params: [
            {
              from: "0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266",
              data: incrementDeploymentBytecode,
            },
          ],
        })
      );

      const transactionHash = JSON.parse(sendTransactionResponse.data).result;
      assert.isDefined(transactionHash, "Transaction failed");

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
      const provider = await createL1Provider(context, {
        ...providerOverrides,
        observability: {
          codeCoverage: {
            onCollectedCoverageCallback: async (_coverage: Uint8Array[]) => {
              throw new Error(ERROR_MESSAGE);
            },
          },
        },
      });

      const sendTransactionResponse = await provider.handleRequest(
        JSON.stringify({
          id: 1,
          jsonrpc: "2.0",
          method: "eth_sendTransaction",
          params: [
            {
              from: "0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266",
              data: incrementDeploymentBytecode,
            },
          ],
        })
      );

      const error = JSON.parse(sendTransactionResponse.data).error;

      assert(
        error.message.includes(ERROR_MESSAGE),
        "Error message should contain the expected error"
      );
      assert.equal(error.code, -32603);
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
        disableTransactionGasCap: true,
        projectRoot: __dirname,
        hardfork: l1HardforkToString(l1HardforkLatest()),
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
        disableTransactionGasCap: true,
        projectRoot: __dirname,
        hardfork: l1HardforkToString(l1HardforkLatest()),
        observability: {
          codeCoverage: {
            onCollectedCoverageCallback: async (_coverage: Uint8Array[]) => {
              throw new Error(ERROR_MESSAGE);
            },
          },
        },
      };

      const [, suiteResults] = await runAllSolidityTests(
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
