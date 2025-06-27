import assert from "node:assert/strict";
import { before, describe, it } from "node:test";
import { Interface } from "ethers";
import { toBytes } from "@nomicfoundation/ethereumjs-util";
import edr from "@nomicfoundation/edr";

import { TestContext } from "./testContext.js";
import {
    L1_CHAIN_TYPE,
    l1GenesisState,
    l1HardforkLatest,
    l1HardforkToString,
    MineOrdering,
    SubscriptionEvent,
} from "@nomicfoundation/edr";

describe("Provider tests", () => {
    let testContext: TestContext;

    before(async () => {
        testContext = await TestContext.setup();
    });

    it("CustomPrecompileEnabled", async function () {
        const customPrecompileArtifact = testContext.artifacts.find(
            (artifact) => artifact.id.name === "CustomPrecompile"
        );

        assert.notStrictEqual(customPrecompileArtifact, undefined);

        const counterInterface = new Interface(customPrecompileArtifact!.contract.abi);

        const hardfork = l1HardforkLatest();

        const providerConfig = {
            allowBlocksWithSameTimestamp: false,
            allowUnlimitedContractSize: true,
            bailOnCallFailure: false,
            bailOnTransactionFailure: false,
            blockGasLimit: 300_000_000n,
            chainId: 123n,
            chainOverrides: [],
            coinbase: Buffer.from("0000000000000000000000000000000000000000", "hex"),
            genesisState: [
                {
                    address: toBytes("0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"),
                    balance: 1000n * 10n ** 18n,
                },
                ...l1GenesisState(hardfork),
            ],
            hardfork: l1HardforkToString(hardfork),
            initialBlobGas: {
                gasUsed: 0n,
                excessGas: 0n,
            },
            initialParentBeaconBlockRoot: Buffer.from(
                "0000000000000000000000000000000000000000000000000000000000000000",
                "hex"
            ),
            minGasPrice: 0n,
            mining: {
                autoMine: true,
                memPool: {
                    order: MineOrdering.Priority,
                },
            },
            networkId: 123n,
            observability: {
                codeCoverage: {
                    onCollectedCoverageCallback: (coverage: Uint8Array[]) => { },
                },
            },
            ownedAccounts: [
                "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80",
            ],
            precompileOverrides: [(0, edr.precompileP256Verify)()],
        };

        const loggerConfig = {
            enable: false,
            decodeConsoleLogInputsCallback: (_inputs: ArrayBuffer[]): string[] => {
                return [];
            },
            printLineCallback: (_message: string, _replace: boolean) => { },
        };

        const provider = await testContext.edrContext.createProvider(
            L1_CHAIN_TYPE,
            providerConfig,
            loggerConfig,
            {
                subscriptionCallback: (_event: SubscriptionEvent) => { },
            },
            {}
        );

        const sender = "0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266";

        const deploymentTransactionResponse = await provider.handleRequest(
            JSON.stringify({
                id: 1,
                jsonrpc: "2.0",
                method: "eth_sendTransaction",
                params: [
                    {
                        from: sender,
                        data: customPrecompileArtifact!.contract.bytecode,
                    },
                ],
            })
        );

        const deploymentTransactionHash = JSON.parse(
            deploymentTransactionResponse.data
        ).result;

        const deploymentTransactionReceiptResponse = await provider.handleRequest(
            JSON.stringify({
                id: 1,
                jsonrpc: "2.0",
                method: "eth_getTransactionReceipt",
                params: [deploymentTransactionHash],
            })
        );

        const deployedAddress = JSON.parse(
            deploymentTransactionReceiptResponse.data
        ).result.contractAddress;

        const precompileTransactionResponse = await provider.handleRequest(
            JSON.stringify({
                id: 1,
                jsonrpc: "2.0",
                method: "eth_sendTransaction",
                params: [
                    {
                        from: sender,
                        to: deployedAddress,
                        data: counterInterface.encodeFunctionData("rip2712Precompile"),
                    },
                ],
            })
        );

        const precompileTransactionHash = JSON.parse(
            precompileTransactionResponse.data
        ).result;

        const precompileTransactionReceiptResponse = await provider.handleRequest(
            JSON.stringify({
                id: 1,
                jsonrpc: "2.0",
                method: "eth_getTransactionReceipt",
                params: [precompileTransactionHash],
            })
        );

        const precompileReceipt = JSON.parse(
            precompileTransactionReceiptResponse.data
        ).result;
        assert.strictEqual(precompileReceipt.status, "0x1");
    });

    it("CustomPrecompileDisabled", async function () {
        const customPrecompileArtifact = testContext.artifacts.find(
            (artifact) => artifact.id.name === "CustomPrecompile"
        );

        assert.notStrictEqual(customPrecompileArtifact, undefined);

        const counterInterface = new Interface(customPrecompileArtifact!.contract.abi);

        const hardfork = l1HardforkLatest();

        const providerConfig = {
            allowBlocksWithSameTimestamp: false,
            allowUnlimitedContractSize: true,
            bailOnCallFailure: false,
            bailOnTransactionFailure: false,
            blockGasLimit: 300_000_000n,
            chainId: 123n,
            chainOverrides: [],
            coinbase: Buffer.from("0000000000000000000000000000000000000000", "hex"),
            genesisState: [
                {
                    address: toBytes("0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"),
                    balance: 1000n * 10n ** 18n,
                },
                ...l1GenesisState(hardfork),
            ],
            hardfork: l1HardforkToString(hardfork),
            initialBlobGas: {
                gasUsed: 0n,
                excessGas: 0n,
            },
            initialParentBeaconBlockRoot: Buffer.from(
                "0000000000000000000000000000000000000000000000000000000000000000",
                "hex"
            ),
            minGasPrice: 0n,
            mining: {
                autoMine: true,
                memPool: {
                    order: MineOrdering.Priority,
                },
            },
            networkId: 123n,
            observability: {
                codeCoverage: {
                    onCollectedCoverageCallback: (coverage: Uint8Array[]) => { },
                },
            },
            ownedAccounts: [
                "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80",
            ],
            precompileOverrides: [],
        };

        const loggerConfig = {
            enable: false,
            decodeConsoleLogInputsCallback: (_inputs: ArrayBuffer[]): string[] => {
                return [];
            },
            printLineCallback: (_message: string, _replace: boolean) => { },
        };

        const provider = await testContext.edrContext.createProvider(
            L1_CHAIN_TYPE,
            providerConfig,
            loggerConfig,
            {
                subscriptionCallback: (_event: SubscriptionEvent) => { },
            },
            {}
        );

        const sender = "0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266";

        const deploymentTransactionResponse = await provider.handleRequest(
            JSON.stringify({
                id: 1,
                jsonrpc: "2.0",
                method: "eth_sendTransaction",
                params: [
                    {
                        from: sender,
                        data: customPrecompileArtifact!.contract.bytecode,
                    },
                ],
            })
        );

        const deploymentTransactionHash = JSON.parse(
            deploymentTransactionResponse.data
        ).result;

        const deploymentTransactionReceiptResponse = await provider.handleRequest(
            JSON.stringify({
                id: 1,
                jsonrpc: "2.0",
                method: "eth_getTransactionReceipt",
                params: [deploymentTransactionHash],
            })
        );

        const deployedAddress = JSON.parse(
            deploymentTransactionReceiptResponse.data
        ).result.contractAddress;

        const precompileTransactionResponse = await provider.handleRequest(
            JSON.stringify({
                id: 1,
                jsonrpc: "2.0",
                method: "eth_sendTransaction",
                params: [
                    {
                        from: sender,
                        to: deployedAddress,
                        data: counterInterface.encodeFunctionData("rip2712Precompile"),
                    },
                ],
            })
        );

        const precompileTransactionHash = JSON.parse(
            precompileTransactionResponse.data
        ).result;

        const precompileTransactionReceiptResponse = await provider.handleRequest(
            JSON.stringify({
                id: 1,
                jsonrpc: "2.0",
                method: "eth_getTransactionReceipt",
                params: [precompileTransactionHash],
            })
        );

        const precompileReceipt = JSON.parse(
            precompileTransactionReceiptResponse.data
        ).result;
        assert.strictEqual(precompileReceipt.status, "0x0");
    });
});