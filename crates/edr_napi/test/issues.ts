import chai, { assert } from "chai";
import { JsonStreamStringify } from "json-stream-stringify";

import {
  ContractAndFunctionName,
  EdrContext,
  MineOrdering,
  Provider,
  SpecId,
  SubscriptionEvent,
} from "..";
import { ALCHEMY_URL, isCI } from "./helpers";

describe("Provider", () => {
  const context = new EdrContext();
  const providerConfig = {
    allowBlocksWithSameTimestamp: false,
    allowUnlimitedContractSize: true,
    bailOnCallFailure: false,
    bailOnTransactionFailure: false,
    blockGasLimit: 300_000_000n,
    chainId: 1n,
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
    hardfork: SpecId.Latest,
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

  it("issue 543", async function () {
    if (ALCHEMY_URL === undefined || !isCI()) {
      this.skip();
    }

    // This test is slow because the debug_traceTransaction is performed on a large transaction.
    this.timeout(240_000);

    const provider = await Provider.withConfig(
      context,
      {
        fork: {
          jsonRpcUrl: ALCHEMY_URL,
        },
        initialBaseFeePerGas: 0n,
        ...providerConfig,
      },
      loggerConfig,
      (_event: SubscriptionEvent) => {},
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
});
