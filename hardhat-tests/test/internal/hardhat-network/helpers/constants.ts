// reused from ethers.js
import { Address, toBytes } from "@nomicfoundation/ethereumjs-util";
import path from "path";

function toBuffer(x: Parameters<typeof toBytes>[0]) {
  return Buffer.from(toBytes(x));
}

export const DAI_ADDRESS = Address.fromString(
  "0x6b175474e89094c44da98b954eedeac495271d0f"
);

export const WETH_ADDRESS = Address.fromString(
  "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"
);

export const UNISWAP_FACTORY_ADDRESS = Address.fromString(
  "0xc0a47dFe034B400B47bDaD5FecDa2621de6c4d95"
);

export const EMPTY_ACCOUNT_ADDRESS = Address.fromString(
  "0x246a566a96ae9fa8dcf04d4c6c094c7c492f018f"
);

// top Ether holder as of 24.08.2020
export const BITFINEX_WALLET_ADDRESS = Address.fromString(
  "0x742d35Cc6634C0532925a3b844Bc454e4438f44e"
);

// 10496585 block number was chosen for no particular reason
export const BLOCK_NUMBER_OF_10496585 = 10496585n;
export const FIRST_TX_HASH_OF_10496585 = toBuffer(
  "0xed0b0b132bd693ef34a72084f090df07c5c3a2ec019d76316da040d4222cdfb8"
);

export const FORK_TESTS_CACHE_PATH = path.join(
  __dirname,
  "..",
  "provider",
  ".hardhat_node_test_cache"
);
