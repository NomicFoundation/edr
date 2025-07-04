import { toBytes } from "@nomicfoundation/ethereumjs-util";
import { JsonStreamStringify } from "json-stream-stringify";
import fs from "fs";

import {
  AccountOverride,
  GENERIC_CHAIN_TYPE,
  genericChainProviderFactory,
  l1HardforkLatest,
  l1HardforkToString,
  MineOrdering,
  SubscriptionEvent,
} from "..";
import { getContext } from "./helpers";

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
    const fileContent = fs.readFileSync("test/files/issue-543.json", "utf-8");
    var parsedJson = JSON.parse(fileContent);
    var structLog = parsedJson.structLogs[0];

    // This creates a JSON of length ~950 000 000 characters.
    // JSON.stringify() crashes at ~500 000 000 characters.
    for (let i = 1; i < 20000; i++) {
      parsedJson.structLogs.push(structLog);
    }

    this.timeout(500_000);

    const provider = await context.createProvider(
      GENERIC_CHAIN_TYPE,
      providerConfig,
      loggerConfig,
      {
        subscriptionCallback: (_event: SubscriptionEvent) => {},
      },
      {}
    );
    // Ignore this on testNoBuild
    // @ts-ignore
    provider.useMockProvider(parsedJson);

    // This is a transaction that has a very large response.
    // It won't be used, the provider will return the mocked response.
    const debugTraceTransaction = `{
        "jsonrpc": "2.0",
        "method": "debug_traceTransaction",
        "params": ["0x7e460f200343e5ab6653a8857cc5ef798e3f5bea6a517b156f90c77ef311a57c"],
        "id": 1
      }`;

    const response = await provider.handleRequest(debugTraceTransaction);

    let responseData = response;

    if (typeof response.data === "string") {
      responseData = JSON.parse(response.data);
    } else {
      responseData = response.data;
    }

    // Validate that we can query the response data without crashing.
    const _json = new JsonStreamStringify(responseData);
  });
});
