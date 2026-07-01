import { toBytes } from "@nomicfoundation/ethereumjs-util";
import {
  Artifact,
  ArtifactId,
  ContractData,
  ContractDecoder,
  EdrContext,
  GENERIC_CHAIN_TYPE,
  genericChainProviderFactory,
  l1GenesisState,
  l1HardforkFromString,
  l1HardforkLatest,
  l1HardforkToString,
  LoggerConfig,
  MineOrdering,
  Provider,
  ProviderConfig,
  SolidityTestResult,
  SolidityTestRunnerConfigArgs,
  SubscriptionEvent,
  SuiteResult,
  TracingMessage,
  TracingMessageResult,
  TracingStep,
} from "..";

function getEnv(key: string): string | undefined {
  const variable = process.env[key];
  if (variable === undefined || variable === "") {
    return undefined;
  }

  const trimmed = variable.trim();

  return trimmed.length === 0 ? undefined : trimmed;
}

export const ALCHEMY_URL = getEnv("ALCHEMY_URL");

export function isCI(): boolean {
  return getEnv("CI") === "true";
}

let globalContext: EdrContext | undefined;

export function getContext(): EdrContext {
  if (globalContext === undefined) {
    globalContext = new EdrContext();
  }
  return globalContext;
}

/**
 * Given a trace, return only its steps.
 */
export function collectSteps(
  trace: Array<TracingMessage | TracingStep | TracingMessageResult>
): TracingStep[] {
  return trace.filter((traceItem) => "pc" in traceItem) as TracingStep[];
}

/**
 * Given a trace, return only its messages.
 */
export function collectMessages(
  trace: Array<TracingMessage | TracingStep | TracingMessageResult>
): TracingMessage[] {
  return trace.filter(
    (traceItem) => "isStaticCall" in traceItem
  ) as TracingMessage[];
}

export async function deployContract(
  provider: Provider,
  deploymentCode: string,
  from = "0xbe862ad9abfe6f22bcb087716c7d89a26051f74c"
): Promise<string> {
  const transactionHash = await handleRequest(provider, "eth_sendTransaction", [
    {
      from,
      data: deploymentCode,
      gas: numberToRpcQuantity(6000000),
      value: numberToRpcQuantity(0),
    },
  ]);

  const response = await handleRequest(provider, "eth_getTransactionReceipt", [
    transactionHash,
  ]);

  return response.contractAddress;
}

export async function getBlockNumber(provider: Provider): Promise<number> {
  const response = await handleRequest(provider, "eth_blockNumber");

  return Number(response);
}

export async function getGasPrice(provider: Provider): Promise<bigint> {
  const response = await handleRequest(provider, "eth_gasPrice");

  return BigInt(response);
}

async function handleRequest(
  provider: Provider,
  method: string,
  params: any[] = []
): Promise<any> {
  const responseObject = await provider.handleRequest(
    JSON.stringify({
      id: 1,
      jsonrpc: "2.0",
      method,
      params,
    })
  );

  let response;
  if (typeof responseObject.data === "string") {
    response = JSON.parse(responseObject.data);
  } else {
    response = responseObject.data;
  }

  return response.result;
}

export function toBuffer(x: Parameters<typeof toBytes>[0]) {
  return Buffer.from(toBytes(x));
}

// Load a contract built with Hardhat into a test suite
export function loadContract(artifactPath: string): Artifact {
  const compiledContract = require(artifactPath);

  const id: ArtifactId = {
    name: compiledContract.contractName,
    solcVersion: "0.8.18",
    source: compiledContract.sourceName,
  };

  const contract: ContractData = {
    abi: JSON.stringify(compiledContract.abi),
    bytecode: compiledContract.bytecode,
  };

  return {
    id,
    contract,
  };
}

function numberToRpcQuantity(n: number | bigint): string {
  return `0x${n.toString(16)}`;
}

export async function runAllSolidityTests(
  context: EdrContext,
  chainType: string,
  artifacts: Artifact[],
  testSuites: ArtifactId[],
  configArgs: SolidityTestRunnerConfigArgs
): Promise<[SolidityTestResult, SuiteResult[]]> {
  return new Promise((resolve, reject) => {
    const resultsFromCallback: SuiteResult[] = [];
    let testResult: SolidityTestResult | undefined;
    let isTestComplete = false;

    const tryResolve = () => {
      if (isTestComplete && resultsFromCallback.length === testSuites.length) {
        resolve([testResult!, resultsFromCallback]);
      }
    };

    context
      .runSolidityTests(
        chainType,
        artifacts,
        testSuites,
        configArgs,
        {}, // Empty tracing config
        (suiteResult: SuiteResult) => {
          resultsFromCallback.push(suiteResult);
          tryResolve();
        }
      )
      .then((result) => {
        testResult = result;
        isTestComplete = true;
        tryResolve();
      })
      .catch(reject);
  });
}

export interface SendTxOptions {
  from?: string;
  to?: string;
  gas?: number;
  gasPrice?: number | bigint;
  data?: string;
  nonce?: number;
  value?: number;
}

export async function sendTransaction(
  provider: Provider,
  options?: SendTxOptions
): Promise<string> {
  const gas = options?.gas ?? 21000;
  const price = options?.gasPrice ?? (await getGasPrice(provider));

  const tx = {
    from: options?.from ?? "0x94a48723b9b46b19c72e3091838d0522618b9363",
    to: options?.to ?? "0xce9efd622e568b3a21b19532c77fc76c93c34bd4",
    gas: numberToRpcQuantity(gas),
    gasPrice: numberToRpcQuantity(price),
    data: options?.data,
    nonce:
      options?.nonce !== undefined
        ? numberToRpcQuantity(options.nonce)
        : undefined,
    value:
      options?.value !== undefined
        ? numberToRpcQuantity(options.value)
        : undefined,
  };

  return handleRequest(provider, "eth_sendTransaction", [tx]);
}

export function sleep(ms: number) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

/** Secret key of the default funded account used across provider tests. */
export const DEFAULT_OWNED_ACCOUNT =
  "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";

/** Address corresponding to {@link DEFAULT_OWNED_ACCOUNT}. */
export const DEFAULT_GENESIS_ADDRESS =
  "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266";


/**
 * Builds a local L1 [`ProviderConfig`] with sensible defaults for tests. The
 * hardfork defaults to the latest L1 hardfork.
 *
 * Any field can be overridden. When `genesisState` is not overridden it
 * defaults to the hardfork's required accounts ({@link l1GenesisState}).
 */
export function l1ProviderConfig(
  overrides: Partial<ProviderConfig> = {}
): ProviderConfig {
  const { genesisState, hardfork: hardforkOverride, ...rest } = overrides;
  const hardfork = hardforkOverride ?? l1HardforkToString(l1HardforkLatest());

  return {
    allowBlocksWithSameTimestamp: false,
    allowUnlimitedContractSize: true,
    bailOnCallFailure: false,
    bailOnTransactionFailure: false,
    chainId: 123n,
    coinbase: new Uint8Array(20),
    defaultTransactionGasLimit: 300_000_000n,
    genesisState:
      genesisState ?? l1GenesisState(l1HardforkFromString(hardfork)),
    hardfork,
    initialParentBeaconBlockRoot: new Uint8Array(32),
    minGasPrice: 0n,
    mining: {
      autoMine: true,
      blockGasLimit: 300_000_000n,
      memPool: {
        order: MineOrdering.Priority,
      },
    },
    network: {
      genesisBlobGas: {
        gasUsed: 0n,
        excessGas: 0n,
      },
      genesisBlockGasLimit: 300_000_000n,
    },
    networkId: 123n,
    observability: {},
    ownedAccounts: [DEFAULT_OWNED_ACCOUNT],
    precompileOverrides: [],
    ...rest,
  };
}

/** A [`LoggerConfig`] with logging disabled, for tests that don't inspect logs. */
export function silentLoggerConfig(): LoggerConfig {
  return {
    enable: false,
    decodeConsoleLogInputsCallback: (_inputs: ArrayBuffer[]): string[] => [],
    printLineCallback: (_message: string, _replace: boolean) => {},
  };
}

/**
 * Creates a local generic-chain-type provider configured for L1, using
 * {@link l1ProviderConfig} defaults and a silent logger. The provided `context`
 * must already have the generic provider factory registered.
 */
export function createL1Provider(
  context: EdrContext,
  overrides: Partial<ProviderConfig> = {}
): Promise<Provider> {
  return context.createProvider(
    GENERIC_CHAIN_TYPE,
    l1ProviderConfig(overrides),
    silentLoggerConfig(),
    {
      subscriptionCallback: (_event: SubscriptionEvent) => {},
    },
    new ContractDecoder()
  );
}

/** Registers the generic chain-type provider factory on the given context. */
export async function registerGenericProviderFactory(
  context: EdrContext
): Promise<void> {
  await context.registerProviderFactory(
    GENERIC_CHAIN_TYPE,
    genericChainProviderFactory()
  );
}
