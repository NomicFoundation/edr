import chai, { assert } from "chai";
import chaiAsPromised from "chai-as-promised";

import {
  l1HardforkFromString,
  l1HardforkLatest,
  l1HardforkToString,
  L1Hardfork,
} from "..";
import {
  createGenericProvider,
  getContext,
  registerGenericProviderFactory,
} from "./helpers";

chai.use(chaiAsPromised);

describe("Hardforks", () => {
  const context = getContext();

  before(async () => {
    await registerGenericProviderFactory(context);
  });

  describe("latest L1 hardfork", () => {
    it("is Osaka", () => {
      // Amsterdam is exposed for early access, but its support is incomplete, so
      // it must not become the latest/default hardfork until it is complete and
      // activated on Ethereum Mainnet.
      assert.equal(l1HardforkLatest(), L1Hardfork.Osaka);
    });
  });

  describe("Amsterdam", () => {
    it("is recognized as a valid hardfork", () => {
      assert.equal(l1HardforkFromString("amsterdam"), L1Hardfork.Amsterdam);
      assert.equal(l1HardforkToString(L1Hardfork.Amsterdam), "amsterdam");
    });

    it("can be used to configure a provider", async () => {
      await assert.isFulfilled(
        createGenericProvider(context, { hardfork: "amsterdam" })
      );
    });
  });
});
