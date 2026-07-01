import chai, { assert } from "chai";
import chaiAsPromised from "chai-as-promised";

import {
  AMSTERDAM,
  l1HardforkFromString,
  l1HardforkLatest,
  l1HardforkToString,
  SpecId,
} from "..";
import {
  createL1Provider,
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
      assert.equal(l1HardforkLatest(), SpecId.Osaka);
    });
  });

  describe("Amsterdam", () => {
    it("is recognized as a valid hardfork", () => {
      assert.equal(l1HardforkFromString(AMSTERDAM), SpecId.Amsterdam);
      assert.equal(l1HardforkToString(SpecId.Amsterdam), AMSTERDAM);
    });

    it("can be used to configure a provider", async () => {
      await assert.isFulfilled(
        createL1Provider(context, { hardfork: AMSTERDAM })
      );
    });
  });
});
