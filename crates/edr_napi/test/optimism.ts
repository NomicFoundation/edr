import chai, { assert } from "chai";
import chaiAsPromised from "chai-as-promised";

import {
  ContractAndFunctionName,
  EdrContext,
  L1_CHAIN_TYPE,
  l1GenesisState,
  l1ProviderFactory,
  MineOrdering,
  SubscriptionEvent,
  // HACK: There is no way to exclude tsc type checking for a file from the
  // CLI, so we ignore the error here to allow `pnpm testNoBuild` to pass.
  // @ts-ignore
  OPTIMISM_CHAIN_TYPE,
  // @ts-ignore
  optimismProviderFactory,
  l1HardforkFromString,
  optimismGenesisState,
  optimismHardforkFromString,
} from "..";
import { ALCHEMY_URL, toBuffer } from "./helpers";

chai.use(chaiAsPromised);

describe("Multi-chain", () => {
  const context = new EdrContext();

  before(async () => {
    await context.registerProviderFactory(L1_CHAIN_TYPE, l1ProviderFactory());
    await context.registerProviderFactory(
      OPTIMISM_CHAIN_TYPE,
      optimismProviderFactory()
    );
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
    hardfork: "Latest",
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

  it("initialize local L1 provider", async function () {
    const provider = context.createProvider(
      L1_CHAIN_TYPE,
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

    await assert.isFulfilled(provider);
  });

  it("initialize local Optimism provider", async function () {
    const provider = context.createProvider(
      OPTIMISM_CHAIN_TYPE,
      {
        genesisState: optimismGenesisState(
          optimismHardforkFromString(providerConfig.hardfork)
        ),
        ...providerConfig,
      },
      loggerConfig,
      {
        subscriptionCallback: (_event: SubscriptionEvent) => {},
      },
      {}
    );

    await assert.isFulfilled(provider);
  });

  it("initialize remote Optimism provider", async function () {
    if (ALCHEMY_URL === undefined) {
      this.skip();
    }

    const provider = context.createProvider(
      OPTIMISM_CHAIN_TYPE,
      {
        fork: {
          jsonRpcUrl: ALCHEMY_URL.replace("eth-", "opt-"),
        },
        // TODO: Add support for overriding remote fork state when the local fork is different
        genesisState: [],
        ...providerConfig,
      },
      loggerConfig,
      {
        subscriptionCallback: (_event: SubscriptionEvent) => {},
      },
      {}
    );

    await assert.isFulfilled(provider);
  });

  describe("Optimism", () => {
    it("eth_getBlockByNumber", async function () {
      // Block with Optimism-specific transaction type
      const BLOCK_NUMBER = 117_156_000;

      const provider = await context.createProvider(
        OPTIMISM_CHAIN_TYPE,
        {
          genesisState: optimismGenesisState(
            optimismHardforkFromString(providerConfig.hardfork)
          ),
          ...providerConfig,
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
          OPTIMISM_CHAIN_TYPE,
          {
            genesisState: optimismGenesisState(
              optimismHardforkFromString(providerConfig.hardfork)
            ),
            ...providerConfig,
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
          OPTIMISM_CHAIN_TYPE,
          {
            genesisState: optimismGenesisState(
              optimismHardforkFromString(providerConfig.hardfork)
            ),
            ...providerConfig,
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
          OPTIMISM_CHAIN_TYPE,
          {
            genesisState: optimismGenesisState(
              optimismHardforkFromString(providerConfig.hardfork)
            ),
            ...providerConfig,
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
  });
});
