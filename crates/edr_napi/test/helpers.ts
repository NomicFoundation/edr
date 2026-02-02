import { toBytes } from "@nomicfoundation/ethereumjs-util";
import {
  Artifact,
  ArtifactId,
  ContractData,
  EdrContext,
  Provider,
  SolidityTestResult,
  SolidityTestRunnerConfigArgs,
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
      console.log("test");
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
