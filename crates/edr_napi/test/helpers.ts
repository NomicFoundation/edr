import { toBytes } from "@nomicfoundation/ethereumjs-util";
import {
  Artifact,
  ArtifactId,
  ContractData,
  EdrContext,
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

export async function runAllSolidityTests(
  context: EdrContext,
  chainType: string,
  artifacts: Artifact[],
  testSuites: ArtifactId[],
  configArgs: SolidityTestRunnerConfigArgs
): Promise<SuiteResult[]> {
  return new Promise((resolve, reject) => {
    const resultsFromCallback: SuiteResult[] = [];

    context
      .runSolidityTests(
        chainType,
        artifacts,
        testSuites,
        configArgs,
        {}, // Empty tracing config
        (suiteResult: SuiteResult) => {
          resultsFromCallback.push(suiteResult);
          if (resultsFromCallback.length === artifacts.length) {
            resolve(resultsFromCallback);
          }
        }
      )
      .catch(reject);
  });
}
