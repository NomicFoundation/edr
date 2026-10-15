import { toBytes } from "@nomicfoundation/ethereumjs-util";
import chai, { assert } from "chai";
import chaiAsPromised from "chai-as-promised";
import { Interface } from "ethers";

import {
  AccountOverride,
  EdrContext,
  L1_CHAIN_TYPE,
  l1GenesisState,
  l1HardforkLatest,
  l1HardforkToString,
  l1ProviderFactory,
  MineOrdering,
  Provider,
  SubscriptionEvent,
  // HACK: There is no way to exclude tsc type checking for a file from the
  // CLI, so we ignore the error here to allow `pnpm testNoBuild` to pass.
  // @ts-ignore op exports are absent in the testNoBuild build
  OP_CHAIN_TYPE,
  // @ts-ignore op exports are absent in the testNoBuild build
  opGenesisState,
  // @ts-ignore op exports are absent in the testNoBuild build
  opHardforkFromString,
  // @ts-ignore op exports are absent in the testNoBuild build
  opHardforkToString,
  // @ts-ignore op exports are absent in the testNoBuild build
  opLatestHardfork,
  // @ts-ignore op exports are absent in the testNoBuild build
  opProviderFactory,
  // @ts-ignore op exports are absent in the testNoBuild build
  opSolidityTestRunnerFactory,
  ContractDecoder,
} from "..";
import {
  ALCHEMY_URL,
  DEFAULT_GENESIS_ADDRESS,
  loadContract,
  runAllSolidityTests,
  toBuffer,
} from "./helpers";

const L1_BLOCK_ADDRESS = "0x4200000000000000000000000000000000000015";
const L1_BLOCK_INTERFACE = new Interface([
  "function sequenceNumber() view returns (uint64)",
  "function blobBaseFeeScalar() view returns (uint32)",
  "function baseFeeScalar() view returns (uint32)",
]);
// The scalars written to L1Block's slot 3 by the OP genesis state.
const L1_BLOCK_BASE_FEE_SCALAR = 5227n;
const L1_BLOCK_BLOB_BASE_FEE_SCALAR = 1014213n;

chai.use(chaiAsPromised);

describe("Multi-chain", () => {
  const context = new EdrContext();

  before(async () => {
    await context.registerProviderFactory(L1_CHAIN_TYPE, l1ProviderFactory());
    await context.registerProviderFactory(OP_CHAIN_TYPE, opProviderFactory());

    await context.registerSolidityTestRunnerFactory(
      OP_CHAIN_TYPE,
      opSolidityTestRunnerFactory()
    );
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
    chainId: 123n,
    chainOverrides: [],
    coinbase: new Uint8Array(
      Buffer.from("0000000000000000000000000000000000000000", "hex")
    ),
    defaultTransactionGasLimit: 300_000_000n,
    genesisState,
    hardfork: opHardforkToString(opLatestHardfork()),
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

  it("initialize local L1 provider", async function () {
    const provider = context.createProvider(
      L1_CHAIN_TYPE,
      {
        ...providerConfig,
        hardfork: l1HardforkToString(l1HardforkLatest()),
        genesisState: providerConfig.genesisState.concat(
          l1GenesisState(l1HardforkLatest())
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

  it("initialize local OP provider", async function () {
    const provider = context.createProvider(
      OP_CHAIN_TYPE,
      {
        ...providerConfig,
        genesisState: opGenesisState(
          opHardforkFromString(providerConfig.hardfork)
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

  it("initialize remote OP provider", async function () {
    if (ALCHEMY_URL === undefined) {
      this.skip();
    }

    const provider = context.createProvider(
      OP_CHAIN_TYPE,
      {
        ...providerConfig,
        network: {
          url: ALCHEMY_URL.replace("eth-", "opt-"),
        },
        // TODO: Add support for overriding remote fork state when the local fork is different
        genesisState: [],
      },
      loggerConfig,
      {
        subscriptionCallback: (_event: SubscriptionEvent) => {},
      },
      new ContractDecoder()
    );

    await assert.isFulfilled(provider);
  });

  describe("OP", () => {
    it("eth_getBlockByNumber", async function () {
      // Block with OP-specific transaction type
      const BLOCK_NUMBER = 117_156_000;

      const provider = await context.createProvider(
        OP_CHAIN_TYPE,
        {
          ...providerConfig,
          genesisState: opGenesisState(
            opHardforkFromString(providerConfig.hardfork)
          ),
        },
        loggerConfig,
        {
          subscriptionCallback: (_event: SubscriptionEvent) => {},
        },
        new ContractDecoder()
      );

      const block = provider.handleRequest(
        JSON.stringify({
          id: 1,
          jsonrpc: "2.0",
          method: "eth_getBlockByNumber",
          params: [toBuffer(BLOCK_NUMBER), false],
        })
      );

      await assert.isFulfilled(block);
    });

    describe("Predeploys", () => {
      it("should have the GasPriceOracle predeploy", async function () {
        const provider = await context.createProvider(
          OP_CHAIN_TYPE,
          {
            ...providerConfig,
            genesisState: providerConfig.genesisState.concat(
              opGenesisState(opHardforkFromString(providerConfig.hardfork))
            ),
          },
          loggerConfig,
          {
            subscriptionCallback: (_event: SubscriptionEvent) => {},
          },
          new ContractDecoder()
        );

        const response = await provider.handleRequest(
          JSON.stringify({
            id: 1,
            jsonrpc: "2.0",
            method: "eth_call",
            params: [
              {
                to: "0x420000000000000000000000000000000000000F",
                data: "0x960e3a23", // isFjord()
              },
            ],
          })
        );
        const responseData = JSON.parse(response.data);

        assert.equal(
          responseData.result,
          "0x0000000000000000000000000000000000000000000000000000000000000001"
        );
      });

      it("should have the L1Block predeploy", async function () {
        const provider = await context.createProvider(
          OP_CHAIN_TYPE,
          {
            ...providerConfig,
            genesisState: providerConfig.genesisState.concat(
              opGenesisState(opHardforkFromString(providerConfig.hardfork))
            ),
          },
          loggerConfig,
          {
            subscriptionCallback: (_event: SubscriptionEvent) => {},
          },
          new ContractDecoder()
        );

        const response = await provider.handleRequest(
          JSON.stringify({
            id: 1,
            jsonrpc: "2.0",
            method: "eth_call",
            params: [
              {
                to: "0x4200000000000000000000000000000000000015",
                data: "0x5cf24969", // basefee()
              },
            ],
          })
        );
        const responseData = JSON.parse(response.data);

        assert.equal(
          responseData.result,
          "0x00000000000000000000000000000000000000000000000000000002540be400"
        ); // 10 gwei
      });

      function createOpProvider() {
        return context.createProvider(
          OP_CHAIN_TYPE,
          {
            ...providerConfig,
            genesisState: providerConfig.genesisState.concat(
              opGenesisState(opHardforkFromString(providerConfig.hardfork))
            ),
          },
          loggerConfig,
          {
            subscriptionCallback: (_event: SubscriptionEvent) => {},
          },
          new ContractDecoder()
        );
      }

      async function request(
        provider: Provider,
        method: string,
        params: any[]
      ): Promise<any> {
        const response = await provider.handleRequest(
          JSON.stringify({ id: 1, jsonrpc: "2.0", method, params })
        );
        return JSON.parse(response.data).result;
      }

      // Slot 3 of L1Block packs `sequenceNumber` (uint64), `blobBaseFeeScalar`
      // (uint32) and `baseFeeScalar` (uint32). Reading them through the
      // getters checks the genesis storage against the layout the bytecode
      // expects.
      it("should initialize the L1Block fee scalars in the contract's storage layout", async function () {
        const provider = await createOpProvider();

        const callGetter = async (name: string): Promise<bigint> => {
          const result = await request(provider, "eth_call", [
            {
              to: L1_BLOCK_ADDRESS,
              data: L1_BLOCK_INTERFACE.encodeFunctionData(name),
            },
          ]);
          const [value] = L1_BLOCK_INTERFACE.decodeFunctionResult(name, result);
          return value;
        };

        assert.equal(await callGetter("sequenceNumber"), 0n);
        assert.equal(
          await callGetter("blobBaseFeeScalar"),
          L1_BLOCK_BLOB_BASE_FEE_SCALAR
        );
        assert.equal(
          await callGetter("baseFeeScalar"),
          L1_BLOCK_BASE_FEE_SCALAR
        );
      });

      it("should charge the L1 data fee with the genesis L1Block scalars", async function () {
        const provider = await createOpProvider();

        // Receipts only carry L1 block info for typed (non-legacy)
        // transactions, so leave the gas price unset to get an EIP-1559 one.
        const transactionHash = await request(provider, "eth_sendTransaction", [
          {
            from: DEFAULT_GENESIS_ADDRESS,
            to: "0x000000000000000000000000000000000000dEaD",
            data: `0x${"ab".repeat(2000)}`,
            gas: "0x249f0",
          },
        ]);
        const receipt = await request(provider, "eth_getTransactionReceipt", [
          transactionHash,
        ]);

        assert.equal(BigInt(receipt.l1BaseFeeScalar), L1_BLOCK_BASE_FEE_SCALAR);
        assert.equal(
          BigInt(receipt.l1BlobBaseFeeScalar),
          L1_BLOCK_BLOB_BASE_FEE_SCALAR
        );
        assert.notEqual(BigInt(receipt.l1Fee), 0n);
      });

      it("should stub unimplemented predeploys", async function () {
        const provider = await context.createProvider(
          OP_CHAIN_TYPE,
          {
            ...providerConfig,
            genesisState: providerConfig.genesisState.concat(
              opGenesisState(opHardforkFromString(providerConfig.hardfork))
            ),
          },
          loggerConfig,
          {
            subscriptionCallback: (_event: SubscriptionEvent) => {},
          },
          new ContractDecoder()
        );

        const response = await provider.handleRequest(
          JSON.stringify({
            id: 1,
            jsonrpc: "2.0",
            method: "eth_call",
            params: [
              {
                to: "0x4200000000000000000000000000000000000016", // L2ToL1MessagePasser
                data: "0x3f827a5a", // MESSAGE_VERSION()
              },
            ],
          })
        );
        const responseData = JSON.parse(response.data);

        assert.equal(
          responseData.result,
          // Error("Predeploy L2ToL1MessagePasser is not supported.")
          "0x08c379a00000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000002f5072656465706c6f79204c32546f4c314d657373616765506173736572206973206e6f7420737570706f727465642e0000000000000000000000000000000000"
        );
      });
    });

    describe("Solidity Tests", () => {
      it("executes tests for OP chain", async function () {
        const artifacts = [
          loadContract("./data/artifacts/default/SetupConsistencyCheck.json"),
          loadContract("./data/artifacts/default/PaymentFailureTest.json"),
        ];
        // All artifacts are test suites.
        const testSuites = artifacts.map((artifact) => artifact.id);
        const config = {
          projectRoot: __dirname,
          hardfork: opHardforkToString(opLatestHardfork()),
        };

        const [, results] = await runAllSolidityTests(
          context,
          OP_CHAIN_TYPE,
          artifacts,
          testSuites,
          config
        );

        assert.equal(results.length, artifacts.length);

        for (const res of results) {
          if (res.id.name.includes("SetupConsistencyCheck")) {
            assert.equal(res.testResults.length, 2);
            assert.equal(res.testResults[0].status, "Success");
            assert.equal(res.testResults[1].status, "Success");
          } else if (res.id.name.includes("PaymentFailureTest")) {
            assert.equal(res.testResults.length, 1);
            assert.equal(res.testResults[0].status, "Failure");
          } else {
            assert.fail("Unexpected test suite name: " + res.id.name);
          }
        }
      });
    });
  });
});
