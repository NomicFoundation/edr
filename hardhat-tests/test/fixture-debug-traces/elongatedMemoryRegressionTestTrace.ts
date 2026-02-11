import { RpcDebugTraceOutput } from "hardhat/internal/hardhat-network/provider/output";

// Trace generated using Geth 1.16.7
//
// Start the Geth dev node with debug API enabled:
// ```bash
// docker run -it --rm -p 8545:8545 -p 8546:8546 ethereum/client-go:stable --dev --http --http.addr 0.0.0.0 --http.api eth,net,debug --ws --ws.addr 0.0.0.0 --ws.api eth,net,debug
// ```
//
// Add the following code to `scripts/simulate-geth.mts`:
// ```ts
// import * as viem from "viem";
//
// const url = "http://localhost:8546"; // For WSL: "http://10.0.0.43:8545";
// const data = `0x3d61000480600b3d3981f35F5F525F`;
//
// const publicClient = viem
//   .createPublicClient({
//     transport: viem.http(url),
//   })
//   .extend((client) => ({
//     async traceTransaction(args: viem.Hash) {
//       return client.request({
//         method: "debug_traceTransaction",
//         params: [
//           args,
//           {
//             enableMemory: true,
//           } as any,
//         ],
//       });
//     },
//   }));
// const walletClient = viem.createWalletClient({
//   transport: viem.http(url),
// });
//
// const [gethAccount] = await walletClient.getAddresses();
//
// const gethClient = viem.createWalletClient({
//   transport: viem.http(url),
//   account: gethAccount,
// });
//
// const deploymentTx = await gethClient.sendTransaction({
//   data,
//   chain: null,
// });
//
// const receipt = await publicClient.waitForTransactionReceipt({
//   hash: deploymentTx,
// });
//
// const contractAddress = receipt.contractAddress;
// const txHash = await gethClient.sendTransaction({
//   to: contractAddress,
//   gas: 6_000_000n,
//   chain: null,
// });
//
// const trace = await publicClient.traceTransaction(txHash);
// console.log("Trace result:", JSON.stringify(trace, null, 2));
// ```
//
// Run the script with:
// ```bash
// npx tsx scripts/simulate-geth.mts &> debug.txt
// ```

export const trace: RpcDebugTraceOutput = {
  gas: 21012,
  failed: false,
  returnValue: "0x",
  structLogs: [
    {
      pc: 0,
      op: "PUSH0",
      gas: 5979000,
      gasCost: 2,
      depth: 1,
      stack: [],
    },
    {
      pc: 1,
      op: "PUSH0",
      gas: 5978998,
      gasCost: 2,
      depth: 1,
      stack: ["0x0"],
    },
    {
      pc: 2,
      op: "MSTORE",
      gas: 5978996,
      gasCost: 6,
      depth: 1,
      stack: ["0x0", "0x0"],
    },
    {
      pc: 3,
      op: "PUSH0",
      gas: 5978990,
      gasCost: 2,
      depth: 1,
      stack: [],
      memory: [
        "0000000000000000000000000000000000000000000000000000000000000000",
      ],
    },
    {
      pc: 4,
      op: "STOP",
      gas: 5978988,
      gasCost: 0,
      depth: 1,
      stack: ["0x0"],
      memory: [
        "0000000000000000000000000000000000000000000000000000000000000000",
      ],
    },
  ],
};
