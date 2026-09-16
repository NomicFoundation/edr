import { assert } from "chai";
import { SigningKey } from "ethers";
import { randomBytes } from "crypto";

import { secp256k1PublicKeyFromSecretKey } from "..";
import { DEFAULT_OWNED_ACCOUNT } from "./helpers";

const CURVE_ORDER = BigInt(
  "0xfffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd0364141"
);

const SECRET_KEY_BYTES = 32;
const UNCOMPRESSED_PUBLIC_KEY_BYTES = 65;
const SEC1_UNCOMPRESSED_TAG = 0x04;
const FUZZ_ITERATIONS = 500;

const INVALID_SECRET_KEY_MESSAGE = new RegExp(
  `Expected a ${SECRET_KEY_BYTES}-byte big-endian scalar in \\[1, n\\)`
);

function toHex(bytes: Uint8Array): string {
  return `0x${Buffer.from(bytes).toString("hex")}`;
}

function secretKeyOf(scalar: bigint): Uint8Array {
  return Uint8Array.from(
    Buffer.from(scalar.toString(16).padStart(SECRET_KEY_BYTES * 2, "0"), "hex")
  );
}

const KNOWN_SECRET_KEY = Buffer.from(
  DEFAULT_OWNED_ACCOUNT.replace(/^0x/, ""),
  "hex"
);
const KNOWN_PUBLIC_KEY =
  "0x048318535b54105d4a7aae60c08fc45f9687181b4fdfc625bd1a753fa7397fed75" +
  "3547f11ca8696646f2f3acb08e31016afac23e630c5d11f59f61fef57b0d2aa5";

describe("secp256k1PublicKeyFromSecretKey", function () {
  it("derives the well-known public key of a known secret key", function () {
    assert.strictEqual(
      toHex(secp256k1PublicKeyFromSecretKey(KNOWN_SECRET_KEY)),
      KNOWN_PUBLIC_KEY
    );
  });

  it("returns a 65-byte uncompressed point", function () {
    const publicKey = secp256k1PublicKeyFromSecretKey(
      secretKeyOf(BigInt(1337))
    );

    assert.instanceOf(publicKey, Uint8Array);
    assert.strictEqual(publicKey.length, UNCOMPRESSED_PUBLIC_KEY_BYTES);
    assert.strictEqual(publicKey[0], SEC1_UNCOMPRESSED_TAG);
  });

  it("accepts the ends of the valid range", function () {
    for (const scalar of [BigInt(1), CURVE_ORDER - BigInt(1)]) {
      const secretKey = secretKeyOf(scalar);

      assert.strictEqual(
        toHex(secp256k1PublicKeyFromSecretKey(secretKey)),
        new SigningKey(secretKey).publicKey
      );
    }
  });

  it("rejects secret keys outside the valid range", function () {
    for (const scalar of [
      BigInt(0),
      CURVE_ORDER,
      CURVE_ORDER + BigInt(1),
      BigInt(2) ** BigInt(256) - BigInt(1),
    ]) {
      assert.throws(
        () => secp256k1PublicKeyFromSecretKey(secretKeyOf(scalar)),
        INVALID_SECRET_KEY_MESSAGE,
        `the scalar ${scalar} should be rejected`
      );
    }
  });

  it("rejects inputs that aren't 32 bytes", function () {
    for (const length of [
      0,
      1,
      SECRET_KEY_BYTES - 1,
      SECRET_KEY_BYTES + 1,
      SECRET_KEY_BYTES * 2,
      UNCOMPRESSED_PUBLIC_KEY_BYTES,
    ]) {
      assert.throws(
        () => secp256k1PublicKeyFromSecretKey(new Uint8Array(length).fill(1)),
        INVALID_SECRET_KEY_MESSAGE,
        `a ${length}-byte input should be rejected`
      );
    }
  });

  it("accepts a view into a pooled Buffer", function () {
    // Padded on both sides, so the view starts at a non-zero `byteOffset`.
    const padding = 8;
    const pool = Buffer.alloc(padding + SECRET_KEY_BYTES + padding);
    const pooled = pool.subarray(padding, padding + SECRET_KEY_BYTES);
    KNOWN_SECRET_KEY.copy(pooled);
    assert.notStrictEqual(pooled.byteOffset, 0);

    assert.strictEqual(
      toHex(secp256k1PublicKeyFromSecretKey(pooled)),
      KNOWN_PUBLIC_KEY
    );
  });

  // A mismatch with EthersJS implementation would produce wrong addresses, so
  // this covers a wide range of random keys.
  it("fuzz: matches the EthersJS implementation for random secret keys", function () {
    for (let i = 0; i < FUZZ_ITERATIONS; i++) {
      const secretKey = randomBytes(SECRET_KEY_BYTES);

      let expected;
      try {
        expected = new SigningKey(secretKey).publicKey;
      } catch {
        // Outside [1, n), which the cases above cover explicitly.
        continue;
      }

      assert.strictEqual(
        toHex(secp256k1PublicKeyFromSecretKey(secretKey)),
        expected,
        `mismatch for ${toHex(secretKey)}`
      );
    }
  });
});
