import chai, { assert, expect } from "chai";
import chaiAsPromised from "chai-as-promised";
import * as fs from "fs";

import {
  ContractAndFunctionName,
  GENERIC_CHAIN_TYPE,
  genericChainProviderFactory,
  l1GenesisState,
  l1HardforkFromString,
  l1HardforkLatest,
  l1HardforkToString,
  MineOrdering,
  SubscriptionEvent,
} from "..";
import { getContext } from "./helpers";

chai.use(chaiAsPromised);

class CoverageReporter {
  public hits: Buffer[] = [];
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

  let coverageReporter: CoverageReporter;
  before(async () => {
    await context.registerProviderFactory(
      GENERIC_CHAIN_TYPE,
      genericChainProviderFactory()
    );

    // Reset the coverage reporter
    coverageReporter = new CoverageReporter();
  });

  const providerConfig = {
    allowBlocksWithSameTimestamp: false,
    allowUnlimitedContractSize: true,
    bailOnCallFailure: false,
    bailOnTransactionFailure: false,
    blockGasLimit: 300_000_000n,
    chainId: 123n,
    chains: [],
    coinbase: Buffer.from("0000000000000000000000000000000000000000", "hex"),
    enableRip7212: false,
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
    ownedAccounts: [
      {
        secretKey:
          "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80",
        balance: 1000n * 10n ** 18n,
      },
    ],
    observability: {
      codeCoverage: {
        onCollectedCoverageCallback: (coverage: Buffer[]) => {
          coverageReporter.hits.push(...coverage);
        },
      },
    },
  };

  const loggerConfig = {
    enable: false,
    decodeConsoleLogInputsCallback: (_inputs: Buffer[]): string[] => {
      return [];
    },
    getContractAndFunctionNameCallback: (
      _code: Buffer,
      _calldata?: Buffer
    ): ContractAndFunctionName => {
      return {
        contractName: "",
      };
    },
    printLineCallback: (_message: string, _replace: boolean) => {},
  };

  describe("eth_sendTransaction", function () {
    it("should report code coverage hits", async function () {
      const provider = await context.createProvider(
        GENERIC_CHAIN_TYPE,
        {
          genesisState: l1GenesisState(
            l1HardforkFromString(providerConfig.hardfork)
          ),
          ...providerConfig,
        },
        loggerConfig,
        {
          subscriptionCallback: (_event: SubscriptionEvent) => {},
        },
        {}
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
  });
});
