import { LegacyTransaction } from "@nomicfoundation/ethereumjs-tx";
import {
  bytesToHex as bufferToHex,
  toBytes,
  zeroAddress,
} from "@nomicfoundation/ethereumjs-util";

import { numberToRpcQuantity } from "hardhat/internal/core/jsonrpc/types/base-types";
import { RpcTransactionRequestInput } from "hardhat/internal/core/jsonrpc/types/input/transactionRequest";
import { EIP1193Provider, EthereumProvider } from "hardhat/types";

import {
  DEFAULT_ACCOUNTS,
  DEFAULT_ACCOUNTS_ADDRESSES,
  DEFAULT_BLOCK_GAS_LIMIT,
} from "./providers";
import { getPendingBaseFeePerGas } from "./getPendingBaseFeePerGas";
import { retrieveCommon } from "./retrieveCommon";

export type AccessListBufferItem = [Uint8Array, Uint8Array[]];

export type TransactionParams =
  | LegacyTransactionParams
  | AccessListTransactionParams
  | EIP1559TransactionParams;

interface BaseTransactionParams {
  // `to` should be undefined for contract creation
  to?: Uint8Array;
  from: Uint8Array;
  gasLimit: bigint;
  value: bigint;
  data: Uint8Array;
  nonce: bigint;
}

export interface LegacyTransactionParams extends BaseTransactionParams {
  gasPrice: bigint;
}

export interface AccessListTransactionParams extends BaseTransactionParams {
  gasPrice: bigint;
  // We use this access list format because @nomicfoundation/ethereumjs-tx access list data
  // forces us to use it or stringify them
  accessList: AccessListBufferItem[];
  // We don't include chainId as it's not necessary, the node
  // already knows its chainId, and the Eth module must validate it
}

export interface EIP1559TransactionParams extends BaseTransactionParams {
  accessList: AccessListBufferItem[];
  maxFeePerGas: bigint;
  maxPriorityFeePerGas: bigint;
}

function toBuffer(x: Parameters<typeof toBytes>[0]) {
  return Buffer.from(toBytes(x));
}

export async function deployContract(
  provider: EthereumProvider,
  deploymentCode: string,
  from = DEFAULT_ACCOUNTS_ADDRESSES[0],
  value = 0
): Promise<string> {
  const hash = await provider.send("eth_sendTransaction", [
    {
      from,
      data: deploymentCode,
      gas: numberToRpcQuantity(DEFAULT_BLOCK_GAS_LIMIT),
      value: numberToRpcQuantity(value),
    },
  ]);

  const { contractAddress } = await provider.send("eth_getTransactionReceipt", [
    hash,
  ]);

  return contractAddress;
}

export async function sendTxToZeroAddress(
  provider: EthereumProvider,
  from?: string
): Promise<string> {
  const accounts = await provider.send("eth_accounts");

  const burnTxParams = {
    from: from ?? accounts[0],
    to: zeroAddress(),
    value: numberToRpcQuantity(1),
    gas: numberToRpcQuantity(21000),
    gasPrice: numberToRpcQuantity(await getPendingBaseFeePerGas(provider)),
  };

  return provider.send("eth_sendTransaction", [burnTxParams]);
}

export async function sendTransactionFromTxParams(
  provider: EthereumProvider,
  txParams: TransactionParams
) {
  const rpcTxParams: RpcTransactionRequestInput = {
    from: bufferToHex(txParams.from),
    data: bufferToHex(txParams.data),
    nonce: numberToRpcQuantity(txParams.nonce),
    value: numberToRpcQuantity(txParams.value),
    gas: numberToRpcQuantity(txParams.gasLimit),
  };

  if ("accessList" in txParams) {
    rpcTxParams.accessList = txParams.accessList.map(
      ([address, storageKeys]) => ({
        address: bufferToHex(address),
        storageKeys: storageKeys.map(bufferToHex),
      })
    );
  }

  if ("gasPrice" in txParams) {
    rpcTxParams.gasPrice = numberToRpcQuantity(txParams.gasPrice);
  } else {
    rpcTxParams.maxFeePerGas = numberToRpcQuantity(txParams.maxFeePerGas);
    rpcTxParams.maxPriorityFeePerGas = numberToRpcQuantity(
      txParams.maxPriorityFeePerGas
    );
  }

  if (txParams.to !== undefined) {
    rpcTxParams.to = bufferToHex(txParams.to!);
  }
  return provider.send("eth_sendTransaction", [rpcTxParams]);
}

export async function getSignedTxHash(
  hardhatNetworkProvider: EIP1193Provider,
  txParams: TransactionParams,
  signerAccountIndex: number
) {
  const txToSign = new LegacyTransaction(txParams, {
    common: await retrieveCommon(hardhatNetworkProvider),
  });

  const signedTx = txToSign.sign(
    toBuffer(DEFAULT_ACCOUNTS[signerAccountIndex].privateKey)
  );

  return bufferToHex(signedTx.hash());
}

/**
 * Returns a transaction that deploys a contract with bytecode `bytecode`.
 *
 * This helper is different from deployContract because that helper receives
 * the deployment bytecode, while this one receives the bytecode that we want
 * to deploy, plus the length of the slice of that bytecode we want deployed.
 */
export function getTxToDeployBytecode(
  bytecode: string,
  bytecodeLength: number = bytecode.length / 2,
  from = DEFAULT_ACCOUNTS_ADDRESSES[0]
) {
  const deployedCodeLengthHex = bytecodeLength.toString(16).padStart(4, "0");

  if (deployedCodeLengthHex.length > 4) {
    throw new Error("This helper can only deploy up to 0xFFFF bytes");
  }

  // 3d: RETURNDATASIZE (pushes 0 to the stack)
  // 61 ${deployedCodeLengthHex}: PUSH2, pushes ${deployedCodeLengthHex} to the
  // stack; this is the length of ${code} that will be used
  // 80: DUP1, duplicates the stack entry with the length of the deployed code
  // 600b: pushes 0b to the stack; this is the position where ${code} starts
  // 3d: RETURNDATASIZE (pushes 0 to the stack)
  // 39: CODECOPY, copies the code to deploy to memory
  // 81: DUP2, duplicates the stack entry with the length of the deployed code
  // F3: RETURN
  const deploymentBytecode = `0x3d61${deployedCodeLengthHex}80600b3d3981f3${bytecode}`;

  return {
    from,
    data: deploymentBytecode,
    gas: numberToRpcQuantity(DEFAULT_BLOCK_GAS_LIMIT),
    gasPrice: numberToRpcQuantity(0),
  };
}

export async function sendDeploymentTx(
  provider: EthereumProvider,
  tx: any
): Promise<string> {
  const hash = await provider.send("eth_sendTransaction", [tx]);

  const { contractAddress } = await provider.send("eth_getTransactionReceipt", [
    hash,
  ]);

  return contractAddress;
}
