const {
  time,
  loadFixture,
} = require("@nomicfoundation/hardhat-toolbox/network-helpers");
const { expect } = require("chai");
const hre = require("hardhat");

describe("HH+EDR and hardhat-tracer", function () {
  // We define a fixture to reuse the same setup in every test.
  // We use loadFixture to run this setup once, snapshot that state,
  // and reset Hardhat Network to that snapshot in every test.
  async function deployOneYearLockFixture() {
    const ONE_YEAR_IN_SECS = 365 * 24 * 60 * 60;
    const ONE_GWEI = 1_000_000_000;

    const lockedAmount = ONE_GWEI;
    const unlockTime = (await time.latest()) + ONE_YEAR_IN_SECS;

    // Contracts are deployed using the first signer/account by default
    const [owner, otherAccount] = await ethers.getSigners();

    const Lock = await ethers.getContractFactory("Lock");
    const lock = await Lock.deploy(unlockTime, { value: lockedAmount });

    return { lock, unlockTime, lockedAmount, owner, otherAccount };
  }
  
  it("verbose tracing returns full stack and memory", async function () {
    const { lock } = await loadFixture(deployOneYearLockFixture);

    const hre = require("hardhat");
    
    // TODO improve accessing EDR provider
    const edrProvider = hre.network.provider.provider._provider._wrappedProvider._wrappedProvider._provider;
    edrProvider.setVerboseTracing(true);

    // TODO check that full stack and memory are returned and that they are in the expected format
    await lock.unlockTime();
  });
});
