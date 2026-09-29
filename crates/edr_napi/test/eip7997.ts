import { bytesToHex } from "@nomicfoundation/ethereumjs-util";
import { assert } from "chai";

import { AccountOverride, L1Hardfork, l1GenesisState, Provider } from "..";
import {
  createGenericProvider,
  getContext,
  registerGenericProviderFactory,
} from "./helpers";

// EIP-7997: Deterministic Factory Contract.
// see <https://eips.ethereum.org/EIPS/eip-7997>
//
// From Amsterdam the genesis state must include the CREATE2 factory with a
// nonzero nonce and the runtime code below.
const FACTORY_ADDRESS = "0x4e59b44847b379578588920ca78fbf26c0b4956c";
const FACTORY_BYTECODE =
  "0x7fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe03601600081602082378035828234f58015156039578182fd5b8082525050506014600cf3";

function factoryOverride(hardfork: L1Hardfork): AccountOverride | undefined {
  return l1GenesisState(hardfork).find(
    (account) => bytesToHex(account.address) === FACTORY_ADDRESS
  );
}

async function getCode(provider: Provider, address: string): Promise<string> {
  const response = await provider.handleRequest(
    JSON.stringify({
      id: 1,
      jsonrpc: "2.0",
      method: "eth_getCode",
      params: [address, "latest"],
    })
  );

  return JSON.parse(response.data).result;
}

describe("EIP-7997 deterministic factory", () => {
  describe("l1GenesisState", () => {
    it("includes the factory with nonce 1 from Amsterdam", () => {
      const factory = factoryOverride(L1Hardfork.Amsterdam);

      assert.isDefined(factory);
      assert.equal(factory.nonce, 1n);
      assert.isDefined(factory.code);
      assert.equal(bytesToHex(factory.code), FACTORY_BYTECODE);
    });

    it("omits the factory before Amsterdam", () => {
      assert.isUndefined(factoryOverride(L1Hardfork.Osaka));
    });
  });

  describe("provider", () => {
    const context = getContext();

    before(async () => {
      await registerGenericProviderFactory(context);
    });

    it("has the factory code on Amsterdam", async () => {
      const provider = await createGenericProvider(context, {
        hardfork: "amsterdam",
      });

      assert.equal(await getCode(provider, FACTORY_ADDRESS), FACTORY_BYTECODE);
    });

    it("has no code at the factory address before Amsterdam", async () => {
      const provider = await createGenericProvider(context, {
        hardfork: "osaka",
      });

      assert.equal(await getCode(provider, FACTORY_ADDRESS), "0x");
    });
  });
});
