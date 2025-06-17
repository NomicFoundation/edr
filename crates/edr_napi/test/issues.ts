import { toBytes } from "@nomicfoundation/ethereumjs-util";
import { assert } from "chai";
import { JsonStreamStringify } from "json-stream-stringify";

import {
  AccountOverride,
  CANCUN,
  GENERIC_CHAIN_TYPE,
  genericChainProviderFactory,
  l1HardforkLatest,
  l1HardforkToString,
  MineOrdering,
  SubscriptionEvent,
} from "..";
import { ALCHEMY_URL, getContext, isCI } from "./helpers";

describe("Provider", () => {
  const context = getContext();

  before(async () => {
    await context.registerProviderFactory(
      GENERIC_CHAIN_TYPE,
      genericChainProviderFactory()
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
    chainId: 1n,
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

  it("issue 543", async function () {
    if (ALCHEMY_URL === undefined || !isCI()) {
      this.skip();
    }

    // This test is slow because the debug_traceTransaction is performed on a large transaction.
    this.timeout(1_800_000);

    const provider = await context.createProvider(
      GENERIC_CHAIN_TYPE,
      {
        ...providerConfig,
        fork: {
          url: ALCHEMY_URL,
        },
        initialBaseFeePerGas: 0n,
      },
      loggerConfig,
      {
        subscriptionCallback: (_event: SubscriptionEvent) => {},
      },
      {}
    );

    const debugTraceTransaction = `{
        "jsonrpc": "2.0",
        "method": "debug_traceTransaction",
        "params": ["0x7e460f200343e5ab6653a8857cc5ef798e3f5bea6a517b156f90c77ef311a57c"],
        "id": 1
      }`;

    const response = await provider.handleRequest(debugTraceTransaction);

    let responseData;

    if (typeof response.data === "string") {
      responseData = JSON.parse(response.data);
    } else {
      responseData = response.data;
    }

    // Validate that we can query the response data without crashing.
    const _json = new JsonStreamStringify(responseData);
  });

  it("issue 771", async function () {
    const provider = await context.createProvider(
      GENERIC_CHAIN_TYPE,
      {
        ...providerConfig,
        initialBaseFeePerGas: 0n,
        mining: {
          // Enable interval mining to validate that provider shutdown works correctly
          interval: 1n,
          ...providerConfig.mining,
        },
      },
      loggerConfig,
      {
        subscriptionCallback: (_event: SubscriptionEvent) => {},
      },
      {}
    );

    // Make a dummy request to ensure the provider constructor doesn't become a no-op
    await provider.handleRequest(
      JSON.stringify({
        id: 1,
        jsonrpc: "2.0",
        method: "eth_blockNumber",
        params: [],
      })
    );
  });
  it("Invalid build info", async function () {
    // Test data taken from CI run:
    // <https://github.com/NomicFoundation/hardhat/actions/runs/14475363227/job/40604573807?pr=6577>
    const provider = context.createProvider(
      GENERIC_CHAIN_TYPE,
      {
        ...providerConfig,
        allowUnlimitedContractSize: false,
        bailOnCallFailure: true,
        bailOnTransactionFailure: true,
        chainId: 31337n,
        coinbase: Buffer.from(
          "c014ba5ec014ba5ec014ba5ec014ba5ec014ba5e",
          "hex"
        ),
        genesisState: [],
        hardfork: CANCUN,
        networkId: 31337n,
      },
      loggerConfig,
      {
        subscriptionCallback: (_event: SubscriptionEvent) => {},
      },
      {
        buildInfos: [
          {
            buildInfo: Uint8Array.from([
              123, 10, 32, 32, 34, 95, 102, 111, 114, 109, 97, 116, 34, 58, 32,
              34, 104, 104, 51, 45, 115, 111, 108, 45, 98, 117, 105, 108, 100,
              45, 105, 110, 102, 111, 45, 49, 34, 44, 10, 32, 32, 34, 105, 100,
              34, 58, 32, 34, 54, 97, 97, 99, 52, 52, 57, 50, 53, 52, 52, 51,
              97, 53, 97, 100, 56, 99, 98, 50, 52, 57, 55, 49, 101, 100, 53, 54,
              54, 51, 97, 56, 101, 98, 55, 101, 50, 56, 57, 56, 34, 44, 10, 32,
              32, 34, 115, 111, 108, 99, 86, 101, 114, 115, 105, 111, 110, 34,
              58, 32, 34, 48, 46, 56, 46, 50, 56, 34, 44, 10, 32, 32, 34, 115,
              111, 108, 99, 76, 111, 110, 103, 86, 101, 114, 115, 105, 111, 110,
              34, 58, 32, 34, 48, 46, 56, 46, 50, 56, 43, 99, 111, 109, 109,
              105, 116, 46, 55, 56, 57, 51, 54, 49, 52, 97, 34, 44, 10, 32, 32,
              34, 112, 117, 98, 108, 105, 99, 83, 111, 117, 114, 99, 101, 78,
              97, 109, 101, 77, 97, 112, 34, 58, 32, 123, 10, 32, 32, 32, 32,
              34, 99, 111, 110, 116, 114, 97, 99, 116, 115, 47, 67, 111, 117,
              110, 116, 101, 114, 46, 115, 111, 108, 34, 58, 32, 34, 99, 111,
              110, 116, 114, 97, 99, 116, 115, 47, 67, 111, 117, 110, 116, 101,
              114, 46, 115, 111, 108, 34, 10, 32, 32, 125, 44, 10, 32, 32, 34,
              105, 110, 112, 117, 116, 34, 58, 32, 123, 10, 32, 32, 32, 32, 34,
              108, 97, 110, 103, 117, 97, 103, 101, 34, 58, 32, 34, 83, 111,
              108, 105, 100, 105, 116, 121, 34, 44, 10, 32, 32, 32, 32, 34, 115,
              101, 116, 116, 105, 110, 103, 115, 34, 58, 32, 123, 10, 32, 32,
              32, 32, 32, 32, 34, 101, 118, 109, 86, 101, 114, 115, 105, 111,
              110, 34, 58, 32, 34, 99, 97, 110, 99, 117, 110, 34, 44, 10, 32,
              32, 32, 32, 32, 32, 34, 111, 117, 116, 112, 117, 116, 83, 101,
              108, 101, 99, 116, 105, 111, 110, 34, 58, 32, 123, 10, 32, 32, 32,
              32, 32, 32, 32, 32, 34, 42, 34, 58, 32, 123, 10, 32, 32, 32, 32,
              32, 32, 32, 32, 32, 32, 34, 42, 34, 58, 32, 91, 10, 32, 32, 32,
              32, 32, 32, 32, 32, 32, 32, 32, 32, 34, 97, 98, 105, 34, 44, 10,
              32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 34, 101, 118, 109,
              46, 98, 121, 116, 101, 99, 111, 100, 101, 34, 44, 10, 32, 32, 32,
              32, 32, 32, 32, 32, 32, 32, 32, 32, 34, 101, 118, 109, 46, 100,
              101, 112, 108, 111, 121, 101, 100, 66, 121, 116, 101, 99, 111,
              100, 101, 34, 44, 10, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32,
              32, 34, 101, 118, 109, 46, 109, 101, 116, 104, 111, 100, 73, 100,
              101, 110, 116, 105, 102, 105, 101, 114, 115, 34, 44, 10, 32, 32,
              32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 34, 109, 101, 116, 97,
              100, 97, 116, 97, 34, 10, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32,
              93, 44, 10, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 34, 34, 58,
              32, 91, 10, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 34,
              97, 115, 116, 34, 10, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 93,
              10, 32, 32, 32, 32, 32, 32, 32, 32, 125, 10, 32, 32, 32, 32, 32,
              32, 125, 44, 10, 32, 32, 32, 32, 32, 32, 34, 114, 101, 109, 97,
              112, 112, 105, 110, 103, 115, 34, 58, 32, 91, 93, 10, 32, 32, 32,
              32, 125, 44, 10, 32, 32, 32, 32, 34, 115, 111, 117, 114, 99, 101,
              115, 34, 58, 32, 123, 10, 32, 32, 32, 32, 32, 32, 34, 99, 111,
              110, 116, 114, 97, 99, 116, 115, 47, 67, 111, 117, 110, 116, 101,
              114, 46, 115, 111, 108, 34, 58, 32, 123, 10, 32, 32, 32, 32, 32,
              32, 32, 32, 34, 99, 111, 110, 116, 101, 110, 116, 34, 58, 32, 34,
              47, 47, 32, 83, 80, 68, 88, 45, 76, 105, 99, 101, 110, 115, 101,
              45, 73, 100, 101, 110, 116, 105, 102, 105, 101, 114, 58, 32, 85,
              78, 76, 73, 67, 69, 78, 83, 69, 68, 92, 110, 112, 114, 97, 103,
              109, 97, 32, 115, 111, 108, 105, 100, 105, 116, 121, 32, 94, 48,
              46, 56, 46, 50, 56, 59, 92, 110, 92, 110, 99, 111, 110, 116, 114,
              97, 99, 116, 32, 67, 111, 117, 110, 116, 101, 114, 32, 123, 92,
              110, 32, 32, 117, 105, 110, 116, 32, 112, 117, 98, 108, 105, 99,
              32, 120, 59, 92, 110, 92, 110, 32, 32, 101, 118, 101, 110, 116,
              32, 73, 110, 99, 114, 101, 109, 101, 110, 116, 40, 117, 105, 110,
              116, 32, 98, 121, 41, 59, 92, 110, 92, 110, 32, 32, 102, 117, 110,
              99, 116, 105, 111, 110, 32, 105, 110, 99, 40, 41, 32, 112, 117,
              98, 108, 105, 99, 32, 123, 92, 110, 32, 32, 32, 32, 120, 43, 43,
              59, 92, 110, 32, 32, 32, 32, 101, 109, 105, 116, 32, 73, 110, 99,
              114, 101, 109, 101, 110, 116, 40, 49, 41, 59, 92, 110, 32, 32,
              125, 92, 110, 92, 110, 32, 32, 102, 117, 110, 99, 116, 105, 111,
              110, 32, 105, 110, 99, 66, 121, 40, 117, 105, 110, 116, 32, 98,
              121, 41, 32, 112, 117, 98, 108, 105, 99, 32, 123, 92, 110, 32, 32,
              32, 32, 114, 101, 113, 117, 105, 114, 101, 40, 98, 121, 32, 62,
              32, 48, 44, 32, 92, 34, 105, 110, 99, 66, 121, 58, 32, 105, 110,
              99, 114, 101, 109, 101, 110, 116, 32, 115, 104, 111, 117, 108,
              100, 32, 98, 101, 32, 112, 111, 115, 105, 116, 105, 118, 101, 92,
              34, 41, 59, 92, 110, 32, 32, 32, 32, 120, 32, 43, 61, 32, 98, 121,
              59, 92, 110, 32, 32, 32, 32, 101, 109, 105, 116, 32, 73, 110, 99,
              114, 101, 109, 101, 110, 116, 40, 98, 121, 41, 59, 92, 110, 32,
              32, 125, 92, 110, 125, 92, 110, 92, 110, 34, 10, 32, 32, 32, 32,
              32, 32, 125, 10, 32, 32, 32, 32, 125, 10, 32, 32, 125, 10, 125,
            ]),
            output: Uint8Array.from([]),
          },
        ],
        ignoreContracts: false,
      }
    );

    await assert.isRejected(
      provider,
      "Failed to parse build info: EOF while parsing a value at line 1 column 0"
    );
  });
});
