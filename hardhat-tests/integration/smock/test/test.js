const { smock } = require("@defi-wonderland/smock");
const { assert } = require("chai");
const chaiAsPromised = require("chai-as-promised");

describe("HH+EDR and smock", function () {
  // Ported from https://github.com/fvictorio/edr-smock-issue
  it("set call override callback for smock", async function () {
    // Hardhat doesn't work with ESM modules, so we have to require it here.
    // The pretest hook runs compile.
    const { ethers } = require("hardhat");

    const mockedFoo = await smock.fake("Foo");
    const Bar = await ethers.getContractFactory("Bar");
    const bar = await Bar.deploy();

    await bar.callFoo(mockedFoo.address);
  });
});
