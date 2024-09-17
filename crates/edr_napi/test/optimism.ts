import chai, { assert } from "chai";
import chaiAsPromised from "chai-as-promised";

import {
  ContractAndFunctionName,
  EdrContext,
  L1_CHAIN_TYPE,
  l1ProviderFactory,
  MineOrdering,
  SubscriptionEvent,
  // HACK: There is no way to exclude tsc type checking for a file from the
  // CLI, so we ignore the error here to allow `pnpm testNoBuild` to pass.
  // @ts-ignore
  OPTIMISM_CHAIN_TYPE,
  // @ts-ignore
  optimismProviderFactory,
} from "..";
import { ALCHEMY_URL, toBuffer } from "./helpers";

chai.use(chaiAsPromised);

describe("Multi-chain", () => {
  const context = new EdrContext();

  before(async () => {
    await context.registerProviderFactory(L1_CHAIN_TYPE, l1ProviderFactory());
    await context.registerProviderFactory(
      OPTIMISM_CHAIN_TYPE,
      optimismProviderFactory(),
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
    genesisAccounts: [
      {
        secretKey:
          "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80",
        balance: 1000n * 10n ** 18n,
      },
    ],
    hardfork: "Latest",
    initialBlobGas: {
      gasUsed: 0n,
      excessGas: 0n,
    },
    initialParentBeaconBlockRoot: Buffer.from(
      "0000000000000000000000000000000000000000000000000000000000000000",
      "hex",
    ),
    minGasPrice: 0n,
    mining: {
      autoMine: true,
      memPool: {
        order: MineOrdering.Priority,
      },
    },
    networkId: 123n,
  };

  const loggerConfig = {
    enable: false,
    decodeConsoleLogInputsCallback: (inputs: Buffer[]): string[] => {
      return [];
    },
    getContractAndFunctionNameCallback: (
      _code: Buffer,
      _calldata?: Buffer,
    ): ContractAndFunctionName => {
      return {
        contractName: "",
      };
    },
    printLineCallback: (message: string, replace: boolean) => {},
  };

  it("initialize L1 provider", async function () {
    const provider = context.createProvider(
      L1_CHAIN_TYPE,
      providerConfig,
      loggerConfig,
      {
        subscriptionCallback: (_event: SubscriptionEvent) => {},
      },
    );

    await assert.isFulfilled(provider);
  });

  it("initialize Optimism provider", async function () {
    const provider = context.createProvider(
      OPTIMISM_CHAIN_TYPE,
      providerConfig,
      loggerConfig,
      {
        subscriptionCallback: (_event: SubscriptionEvent) => {},
      },
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
        ...providerConfig,
      },
      loggerConfig,
      {
        subscriptionCallback: (_event: SubscriptionEvent) => {},
      },
    );

    await assert.isFulfilled(provider);
  });

  describe("Optimism", () => {
    it("eth_getBlockByNumber", async function () {
      // Block with Optimism-specific transaction type
      const BLOCK_NUMBER = 117_156_000;

      const provider = await context.createProvider(
        OPTIMISM_CHAIN_TYPE,
        providerConfig,
        loggerConfig,
        {
          subscriptionCallback: (_event: SubscriptionEvent) => {},
        },
      );

      const block = provider.handleRequest(
        JSON.stringify({
          id: 1,
          jsonrpc: "2.0",
          method: "eth_getBlockByNumber",
          params: [toBuffer(BLOCK_NUMBER), false],
        }),
      );

      await assert.isFulfilled(block);
    });
  });
});
