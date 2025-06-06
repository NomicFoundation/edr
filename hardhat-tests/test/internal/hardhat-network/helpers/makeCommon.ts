import { Common } from "@nomicfoundation/ethereumjs-common";
import { rpcQuantityToNumber } from "hardhat/internal/core/jsonrpc/types/base-types";
import { EthereumProvider } from "hardhat/types";

export async function makeCommon(
  hardhatNetworkProvider: EthereumProvider
): Promise<Common> {
  const chainId = rpcQuantityToNumber(
    await hardhatNetworkProvider.send("eth_chainId", [])
  );

  const networkId = await hardhatNetworkProvider.send("net_version", []);
  const common = Common.custom({
    chainId,
    networkId,
  });

  return common;
}
