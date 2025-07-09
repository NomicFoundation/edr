import { toBytes } from "@nomicfoundation/ethereumjs-util";
import chai, { assert } from "chai";
import chaiAsPromised from "chai-as-promised";

import {
  AccountOverride,
  EdrContext,
  L1_CHAIN_TYPE,
  l1GenesisState,
  l1HardforkLatest,
  l1HardforkToString,
  l1ProviderFactory,
  MineOrdering,
  SubscriptionEvent,
  // HACK: There is no way to exclude tsc type checking for a file from the
  // CLI, so we ignore the error here to allow `pnpm testNoBuild` to pass.
  // @ts-ignore
  OP_CHAIN_TYPE,
  // @ts-ignore
  opGenesisState,
  // @ts-ignore
  opHardforkFromString,
  // @ts-ignore
  opHardforkToString,
  // @ts-ignore
  opLatestHardfork,
  // @ts-ignore
  opProviderFactory,
  // @ts-ignore
  opSolidityTestRunnerFactory,
} from "..";
import {
  ALCHEMY_URL,
  loadContract,
  runAllSolidityTests,
  toBuffer,
} from "./helpers";

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
    blockGasLimit: 300_000_000n,
    chainId: 123n,
    chainOverrides: [],
    coinbase: Buffer.from("0000000000000000000000000000000000000000", "hex"),
    genesisState,
    hardfork: opHardforkToString(opLatestHardfork()),
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
      {}
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
      {}
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
        fork: {
          url: ALCHEMY_URL.replace("eth-", "opt-"),
        },
        // TODO: Add support for overriding remote fork state when the local fork is different
        genesisState: [],
      },
      loggerConfig,
      {
        subscriptionCallback: (_event: SubscriptionEvent) => {},
      },
      {}
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
        {}
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
          {}
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
          {}
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
          {}
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
          loadContract("./artifacts/SetupConsistencyCheck.json"),
          loadContract("./artifacts/PaymentFailureTest.json"),
        ];
        // All artifacts are test suites.
        const testSuites = artifacts.map((artifact) => artifact.id);
        const config = {
          projectRoot: __dirname,
          observability: {},
        };

        const results = await runAllSolidityTests(
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
