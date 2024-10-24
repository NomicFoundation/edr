import { assert } from "chai";

import { rpcQuantityToNumber } from "hardhat/internal/core/jsonrpc/types/base-types";
import { workaroundWindowsCiFailures } from "../../../utils/workaround-windows-ci-failures";
import { setCWD } from "../helpers/cwd";
import { FORKED_PROVIDERS } from "../helpers/providers";

describe("Forking with HTTP headers", function () {
  FORKED_PROVIDERS.forEach(({ rpcProvider, jsonRpcUrl, useProvider }) => {
    workaroundWindowsCiFailures.call(this, { isFork: true });

    let url: string;
    let bearerToken: string;
    const bearerTokenSeparatorIndex = jsonRpcUrl.lastIndexOf("/");
    if (bearerTokenSeparatorIndex !== -1) {
      url = jsonRpcUrl.substring(0, bearerTokenSeparatorIndex);
      bearerToken = jsonRpcUrl.substring(bearerTokenSeparatorIndex + 1);
    }

    describe(`Using ${rpcProvider}`, function () {
      setCWD();

      this.beforeAll(function () {
        // Skip infura because it doesn't support an API key-based bearer token
        if (rpcProvider === "Infura") {
          this.skip();
        }

        // Skip invalid URLs
        if (url === undefined || bearerToken === undefined) {
          this.skip();
        }
      });

      describe("With API key in HTTP headers", function () {
        useProvider({
          forkConfig: {
            jsonRpcUrl: url,
            httpHeaders: {
              Authorization: `Bearer ${bearerToken}`,
            },
          },
        });

        it("Complete JSON-RPC request", async function () {
          const blockNumber = await this.provider.send("eth_blockNumber");
          const minBlockNumber = 10494745; // mainnet block number at 20.07.2020
          assert.isAtLeast(rpcQuantityToNumber(blockNumber), minBlockNumber);
        });
      });
    });
  });
});
