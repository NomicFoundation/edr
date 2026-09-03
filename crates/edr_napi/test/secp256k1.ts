import { assert } from "chai";
import { SigningKey } from "ethers";
import { randomBytes } from "crypto";

import { secp256k1PublicKeyFromSecretKey } from "..";

const CURVE_ORDER = BigInt(
  "0xfffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd0364141"
);

function toHex(bytes: Uint8Array): string {
  return `0x${Buffer.from(bytes).toString("hex")}`;
}

function secretKeyOf(scalar: bigint): Uint8Array {
  return Uint8Array.from(
    Buffer.from(scalar.toString(16).padStart(64, "0"), "hex")
  );
}

const KNOWN_SECRET_KEY =
  "ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";
const KNOWN_PUBLIC_KEY =
  "0x048318535b54105d4a7aae60c08fc45f9687181b4fdfc625bd1a753fa7397fed75" +
  "3547f11ca8696646f2f3acb08e31016afac23e630c5d11f59f61fef57b0d2aa5";

describe("secp256k1PublicKeyFromSecretKey", function () {
  it("derives the well-known public key of a known secret key", function () {
    assert.strictEqual(
      toHex(
        secp256k1PublicKeyFromSecretKey(Buffer.from(KNOWN_SECRET_KEY, "hex"))
      ),
      KNOWN_PUBLIC_KEY
    );
  });

  it("returns a 65-byte uncompressed point", function () {
    const publicKey = secp256k1PublicKeyFromSecretKey(
      secretKeyOf(BigInt(1337))
    );

    assert.instanceOf(publicKey, Uint8Array);
    assert.strictEqual(publicKey.length, 65);
    assert.strictEqual(publicKey[0], 0x04);
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
      assert.throws(() =>
        secp256k1PublicKeyFromSecretKey(secretKeyOf(scalar))
      );
    }
  });

  it("rejects inputs that aren't 32 bytes", function () {
    for (const length of [0, 1, 31, 33, 64, 65]) {
      assert.throws(
        () => secp256k1PublicKeyFromSecretKey(new Uint8Array(length).fill(1)),
        undefined,
        undefined,
        `a ${length}-byte input should be rejected`
      );
    }
  });

  it("accepts a view into a pooled Buffer", function () {
    const pool = Buffer.alloc(96);
    const pooled = pool.subarray(8, 40);
    Buffer.from(KNOWN_SECRET_KEY, "hex").copy(pooled);
    assert.notStrictEqual(pooled.byteOffset, 0);

    assert.strictEqual(
      toHex(secp256k1PublicKeyFromSecretKey(pooled)),
      KNOWN_PUBLIC_KEY
    );
  });

  // A mismatch with the JS implementation would produce wrong addresses, so
  // this covers a wide range of random keys.
  it("fuzz: matches the JS implementation for random secret keys", function () {
    for (let i = 0; i < 500; i++) {
      const secretKey = randomBytes(32);

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
