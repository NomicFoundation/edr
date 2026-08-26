import { assert } from "chai";
import { keccak256 as ethersKeccak256 } from "ethers";
import { randomBytes } from "crypto";

import { keccak256 } from "..";

function toHex(bytes: Uint8Array): string {
  return `0x${Buffer.from(bytes).toString("hex")}`;
}

const EMPTY_INPUT_DIGEST =
  "0xc5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470";

describe("keccak256", function () {
  it("hashes the empty input to the well-known digest", function () {
    assert.strictEqual(toHex(keccak256(new Uint8Array(0))), EMPTY_INPUT_DIGEST);
  });

  it("returns a 32-byte Uint8Array", function () {
    const hash = keccak256(new Uint8Array([0x13, 0x37]));

    assert.instanceOf(hash, Uint8Array);
    assert.strictEqual(hash.length, 32);
  });

  it("accepts a Buffer", function () {
    assert.strictEqual(
      toHex(keccak256(Buffer.from("abc"))),
      "0x4e03657aea45a94fc7d47ba826c8d667c0d1e6e33a64a036ec44f58fa12d6c45"
    );
  });

  // A mismatch with the JS implementation would be consensus-breaking, so this
  // covers every length up to two keccak blocks, the block boundaries
  // themselves, and inputs large enough to need many permutations.
  it("fuzz: matches the JS implementation for random inputs", function () {
    const inputs: Uint8Array[] = [];

    for (let length = 0; length <= 300; length++) {
      inputs.push(randomBytes(length));
    }

    for (const length of [1023, 1024, 1025, 1_500_000]) {
      inputs.push(randomBytes(length));
    }

    for (const input of inputs) {
      assert.strictEqual(
        toHex(keccak256(input)),
        ethersKeccak256(input),
        `mismatch for a ${input.length}-byte input`
      );
    }
  });
});
