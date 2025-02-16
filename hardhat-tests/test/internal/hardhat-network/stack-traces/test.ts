import {
  getLatestSupportedSolcVersion,
  linkHexStringBytecode,
  stackTraceEntryTypeToString,
  TracingConfigWithBuffers,
} from "@nomicfoundation/edr";
import { toBytes } from "@nomicfoundation/ethereumjs-util";
import { assert } from "chai";
import { BUILD_INFO_FORMAT_VERSION } from "hardhat/internal/constants";
import { EdrProviderWrapper } from "hardhat/internal/hardhat-network/provider/provider";
import { ReturnData } from "hardhat/internal/hardhat-network/provider/return-data";
import {
  ConsoleLogs,
  ConsoleLogger,
} from "hardhat/internal/hardhat-network/stack-traces/consoleLogger";
import {
  SolidityStackTrace,
  SolidityStackTraceEntry,
  StackTraceEntryType,
} from "hardhat/internal/hardhat-network/stack-traces/solidity-stack-trace";
import { SUPPORTED_SOLIDITY_VERSION_RANGE } from "hardhat/internal/hardhat-network/stack-traces/constants";
import {
  BuildInfo,
  CompilerInput,
  CompilerOutput,
  CompilerOutputBytecode,
} from "hardhat/types";
import fs from "fs";
import fsExtra from "fs-extra";
import path from "path";
import semver from "semver";

import { setCWD } from "../helpers/cwd";
import { FakeModulesLogger } from "../helpers/fakeLogger";
import {
  compileFiles,
  COMPILER_DOWNLOAD_TIMEOUT,
  downloadCompiler,
} from "./compilation";
import {
  getLatestTestedSolcVersion,
  SolidityCompiler,
  SolidityCompilerOptimizer,
  solidityCompilers,
} from "./compilers-list";
import {
  encodeCall,
  encodeConstructorParams,
  instantiateProvider,
  traceTransaction,
} from "./execution";

interface StackFrameDescription {
  type: string;
  sourceReference?: {
    contract: string;
    file: string;
    function: string;
    line: number;
  };
  message?: string;
  value?: string | number;
  errorCode?: string;
}

interface TestDefinition {
  skip?: boolean;
  skipViaIR?: boolean;
  only?: boolean;
  print?: boolean;
  solc?: string;
  description?: string;
  transactions: TestTransaction[];
}

type TestTransaction = DeploymentTransaction | CallTransaction;

interface DeploymentTransaction {
  file: string;
  contract: string;
  value?: string | number; // Default: 0
  params?: Array<string | number>; // Default: no param
  libraries: {
    [file: string]: {
      [lib: string]: number; // The number of tx that deployed the lib
    };
  };
  stackTrace?: StackFrameDescription[]; // No stack trace === the tx MUST be successful
  imports?: string[]; // Imports needed for successful compilation
  consoleLogs?: ConsoleLogs;
  gas?: number;
}

interface CallTransaction {
  value?: string | number; // Default: 0
  to: number; // The index of the tx that deployed the contract
  stackTrace?: StackFrameDescription[]; // No stack trace === the tx MUST be successful

  // There are two options to pass calldata, the first one (with higher priority) is with data
  data?: string; // 0x-prefixed Hex string

  // The second one is with function and parms
  function?: string; // Default: no data
  params?: Array<string | number>; // Default: no param
  consoleLogs?: ConsoleLogs;
  gas?: number;
}

interface DeployedContract {
  file: string;
  name: string;
  address: Buffer;
}

const TEST_TIMEOUT_MILLIS = 120000;

function defineTest(
  dirPath: string,
  testDefinition: TestDefinition,
  sources: string[],
  compilerOptions: SolidityCompiler
) {
  const desc: string =
    testDefinition.description !== undefined
      ? testDefinition.description
      : path.relative(__dirname, dirPath);

  // test definitions can optionally further restrict the solc version range,
  // if that's the case we skip the test if the current solc version doesn't
  // match the range in the test definition
  const solcVersionDoesntMatch: boolean =
    testDefinition.solc !== undefined &&
    !semver.satisfies(compilerOptions.solidityVersion, testDefinition.solc);

  const skipViaIR =
    testDefinition.skipViaIR === true &&
    compilerOptions.optimizer?.viaIR === true;

  const func = async function (this: Mocha.Context) {
    this.timeout(TEST_TIMEOUT_MILLIS);

    await runTest(dirPath, testDefinition, sources, compilerOptions);
  };

  if (testDefinition.skip === true || skipViaIR || solcVersionDoesntMatch) {
    it.skip(desc, func);
  } else if (testDefinition.only === true) {
    // eslint-disable-next-line mocha/no-exclusive-tests
    it.only(desc, func);
  } else {
    it(desc, func);
  }
}

function defineDirTests(dirPath: string, compilerOptions: SolidityCompiler) {
  describe(path.basename(dirPath), function () {
    const files = fs.readdirSync(dirPath).map((f) => path.join(dirPath, f));

    const sources = files.filter((f) => f.endsWith(".sol"));
    const dirs = files.filter((f) => fs.statSync(f).isDirectory());
    const testPath = path.join(dirPath, "test.json");

    if (fs.existsSync(testPath)) {
      const testDefinition: TestDefinition = JSON.parse(
        fs.readFileSync(testPath, "utf8")
      );

      for (const tx of testDefinition.transactions) {
        if ("imports" in tx && tx.imports !== undefined) {
          sources.push(...tx.imports.map((p: string) => dirPath + p));
          break;
        }
      }

      let description: string;
      if (compilerOptions.optimizer === undefined) {
        description = "Without optimizations";
      } else {
        if (compilerOptions.optimizer.viaIR) {
          description = "With viaIR enabled";
        } else {
          description = `With optimizations (${compilerOptions.optimizer.runs} runs)`;
        }
      }

      describe(description, function () {
        defineTest(dirPath, testDefinition, sources, compilerOptions);
      });
    }

    for (const dir of dirs) {
      defineDirTests(dir, compilerOptions);
    }
  });
}

/** Either re-uses the compiled artifacts or invokes solc to compile the sources.
 *
 * The artifacts are saved in `test-files/artifacts/{testDir}`.
 */
async function compileIfNecessary(
  testDir: string,
  sources: string[],
  compilerOptions: SolidityCompiler
): Promise<[CompilerInput, CompilerOutput]> {
  const { solidityVersion, optimizer } = compilerOptions;
  const maxSourceCtime = sources
    .map((s) => fs.statSync(s).ctimeMs)
    .reduce((t1, t2) => Math.max(t1, t2), 0);

  // save the artifacts in test-files/artifacts/<path-to-test-dir>
  const testFilesDir = path.join(__dirname, "test-files");
  const relativeTestDir = path.relative(testFilesDir, testDir);
  const artifacts = path.join(testFilesDir, "artifacts", relativeTestDir);

  fsExtra.ensureDirSync(artifacts);

  let optimizerModifier: string;
  if (optimizer !== undefined) {
    if (optimizer.viaIR) {
      optimizerModifier = `optimized-with-viair-and-runs-${optimizer.runs}`;
    } else {
      optimizerModifier = `optimized-with-runs-${optimizer.runs}`;
    }
  } else {
    optimizerModifier = "unoptimized";
  }

  const inputPath = path.join(
    artifacts,
    `compiler-input-solc-${solidityVersion}-${optimizerModifier}.json`
  );

  const outputPath = path.join(
    artifacts,
    `compiler-output-solc-${solidityVersion}-${optimizerModifier}.json`
  );

  const isCached =
    fs.existsSync(inputPath) &&
    fs.existsSync(outputPath) &&
    fs.statSync(inputPath).ctimeMs > maxSourceCtime &&
    fs.statSync(outputPath).ctimeMs > maxSourceCtime;

  const usingCustomSolc = process.env.EDR_TESTS_SOLC_PATH !== undefined;

  if (!usingCustomSolc && isCached) {
    const inputJson = fs.readFileSync(inputPath, "utf8");
    const outputJson = fs.readFileSync(outputPath, "utf8");

    return [JSON.parse(inputJson), JSON.parse(outputJson)];
  }

  const [compilerInput, compilerOutput] = await compileFiles(
    sources,
    compilerOptions
  );

  if (!usingCustomSolc) {
    fs.writeFileSync(inputPath, JSON.stringify(compilerInput, undefined, 2));
    fs.writeFileSync(outputPath, JSON.stringify(compilerOutput, undefined, 2));
  }

  return [compilerInput, compilerOutput];
}

function compareStackTraces(
  txIndex: number,
  trace: SolidityStackTraceEntry[],
  description: StackFrameDescription[],
  optimizer: SolidityCompilerOptimizer | undefined
) {
  const isViaIR = optimizer?.viaIR === true;

  // if IR is enabled, we ignore callstack entries in the comparison
  if (isViaIR) {
    trace = trace.filter(
      (frame) => frame.type !== StackTraceEntryType.CALLSTACK_ENTRY
    );
    description = description.filter(
      (frame) => frame.type !== "CALLSTACK_ENTRY"
    );
  }

  if (trace.length !== description.length) {
    console.log(trace);
    console.log(description);
  }

  assert.equal(
    trace.length,
    description.length,
    `Expected a trace of length ${description.length} but got one with length ${trace.length}`
  );

  for (let i = 0; i < trace.length; i++) {
    const actual = trace[i];
    const expected = description[i];

    const actualErrorType = stackTraceEntryTypeToString(actual.type);
    const expectedErrorType = expected.type;

    if (
      isViaIR &&
      (actualErrorType === "REVERT_ERROR" ||
        actualErrorType === "OTHER_EXECUTION_ERROR")
    ) {
      // when viaIR is enabled, we consider a generic REVERT_ERROR or
      // OTHER_EXECUTION_ERROR enough and don't compare its contents
      continue;
    }

    assert.equal(
      actualErrorType,
      expectedErrorType,
      `Stack trace of tx ${txIndex} entry ${i} type is incorrect: expected ${expectedErrorType}, got ${actualErrorType}`
    );

    // actual.message is a ReturnData in revert errors but in custom errors
    // we need to decode it
    const decodedMessage =
      "message" in actual
        ? actual.message
        : "returnData" in actual &&
            new ReturnData(actual.returnData).isErrorReturnData()
          ? new ReturnData(actual.returnData).decodeError()
          : "";

    if (expected.message !== undefined) {
      assert.equal(
        decodedMessage,
        expected.message,
        `Stack trace of tx ${txIndex} entry ${i} have different messages`
      );
    } else {
      assert.equal(
        decodedMessage,
        "",
        `Stack trace of tx ${txIndex} entry ${i} shouldn't have a message`
      );
    }

    if (expected.value !== undefined) {
      const actualValue = "value" in actual ? actual.value : undefined;

      assert.isDefined(
        actualValue,
        `Stack trace of tx ${txIndex} entry ${i} should have value`
      );

      const expectedValue = BigInt(expected.value);

      assert.isTrue(
        expectedValue === actualValue,
        `Stack trace of tx ${txIndex} entry ${i} has value ${actualValue!.toString(
          10
        )} and should have ${expectedValue.toString(10)}`
      );
    } else if ("value" in actual) {
      assert.isUndefined(
        actual.value,
        `Stack trace of tx ${txIndex} entry ${i} shouldn't have value`
      );
    }

    if (expected.errorCode !== undefined) {
      const actualErrorCode =
        "errorCode" in actual ? actual.errorCode : undefined;

      assert.isDefined(
        actualErrorCode,
        `Stack trace of tx ${txIndex} entry ${i} should have an errorCode`
      );

      const actualErrorCodeHex = actualErrorCode!.toString(16);

      assert.isTrue(
        expected.errorCode === actualErrorCodeHex,
        `Stack trace of tx ${txIndex} entry ${i} has errorCode ${actualErrorCodeHex} and should have ${expected.errorCode}`
      );
    } else if ("errorCode" in actual) {
      assert.isUndefined(
        actual.errorCode,
        `Stack trace of tx ${txIndex} entry ${i} shouldn't have errorCode`
      );
    }

    if (expected.sourceReference === undefined) {
      assert.isUndefined(
        actual.sourceReference,
        `Stack trace of tx ${txIndex} entry ${i} shouldn't have a sourceReference`
      );
    } else {
      if (actual.sourceReference === undefined) {
        if (!isViaIR) {
          assert.fail("Expected a source reference");
        }
      } else {
        assert.equal(
          actual.sourceReference.contract,
          expected.sourceReference.contract,
          `Stack trace of tx ${txIndex} entry ${i} have different contract names`
        );

        assert.equal(
          actual.sourceReference.sourceName,
          expected.sourceReference.file,
          `Stack trace of tx ${txIndex} entry ${i} have different file names`
        );

        assert.equal(
          actual.sourceReference.function,
          expected.sourceReference.function,
          `Stack trace of tx ${txIndex} entry ${i} have different function names`
        );

        if (optimizer === undefined) {
          assert.equal(
            actual.sourceReference!.line,
            expected.sourceReference.line,
            `Stack trace of tx ${txIndex} entry ${i} have different line numbers`
          );
        }
      }
    }
  }

  // We do it here so that the first few do get compared
  assert.lengthOf(trace, description.length);
}

function compareConsoleLogs(logs: string[], expectedLogs?: ConsoleLogs) {
  if (expectedLogs === undefined) {
    return;
  }

  assert.lengthOf(logs, expectedLogs.length);

  for (let i = 0; i < logs.length; i++) {
    const actual = logs[i];
    const expected = ConsoleLogger.format(expectedLogs[i]);

    assert.equal(actual, expected);
  }
}

/** The main entry point for a stack trace test. */
async function runTest(
  testDir: string,
  testDefinition: TestDefinition,
  sources: string[],
  compilerOptions: SolidityCompiler
) {
  const [compilerInput, compilerOutput] = await compileIfNecessary(
    testDir,
    sources,
    compilerOptions
  );

  const buildInfo: BuildInfo = {
    id: "stack-traces-test",
    _format: BUILD_INFO_FORMAT_VERSION,
    solcVersion: compilerOptions.solidityVersion,
    solcLongVersion: compilerOptions.solidityVersion,
    input: compilerInput,
    output: compilerOutput,
  };

  const tracingConfig: TracingConfigWithBuffers = {
    buildInfos: [Buffer.from(JSON.stringify(buildInfo))],
    ignoreContracts: true,
  };

  const logger = new FakeModulesLogger();
  const provider = await instantiateProvider(
    {
      enabled: false,
      printLineFn: logger.printLineFn(),
      replaceLastLineFn: logger.replaceLastLineFn(),
    },
    tracingConfig
  );

  const txIndexToContract: Map<number, DeployedContract> = new Map();

  for (const [txIndex, tx] of testDefinition.transactions.entries()) {
    let stackTrace: SolidityStackTrace | undefined;

    if ("file" in tx) {
      const stackTraceOrContractAddress = await runDeploymentTransactionTest(
        txIndex,
        tx,
        provider,
        compilerOutput,
        txIndexToContract
      );

      if (typeof stackTraceOrContractAddress === "string") {
        txIndexToContract.set(txIndex, {
          file: tx.file,
          name: tx.contract,
          address: Buffer.from(stackTraceOrContractAddress, "hex"),
        });
      } else {
        stackTrace = stackTraceOrContractAddress;
      }
    } else {
      const contract = txIndexToContract.get(tx.to);

      assert.isDefined(
        contract,
        `No contract was deployed in tx ${tx.to} but transaction ${txIndex} is trying to call it`
      );

      stackTrace = await runCallTransactionTest(
        txIndex,
        tx,
        provider,
        compilerOutput,
        contract!
      );
    }

    if (tx.stackTrace === undefined) {
      if (stackTrace !== undefined) {
        assert.fail(`Transaction ${txIndex} shouldn't have failed`);
      }
    } else {
      assert.isFalse(
        stackTrace === undefined,
        `Transaction ${txIndex} should have failed`
      );
    }

    if (stackTrace !== undefined) {
      compareStackTraces(
        txIndex,
        stackTrace,
        tx.stackTrace!,
        compilerOptions.optimizer
      );
      if (testDefinition.print !== undefined && testDefinition.print) {
        console.log(`Transaction ${txIndex} stack trace`);
      }
    }

    compareConsoleLogs(logger.lines, tx.consoleLogs);
  }
}

function linkBytecode(
  txIndex: number,
  bytecode: CompilerOutputBytecode,
  libs: { [file: string]: { [lib: string]: number } },
  txIndexToContract: Map<number, DeployedContract>
): Buffer {
  let code = bytecode.object;

  for (const [file, fileLibs] of Object.entries<any>(bytecode.linkReferences)) {
    assert.isDefined(
      libs,
      `Libraries missing for deploying transaction ${txIndex}`
    );

    assert.isDefined(
      libs[file],
      `Libraries missing for deploying transaction ${txIndex}`
    );

    for (const [libName, references] of Object.entries<any>(fileLibs)) {
      assert.isDefined(
        libs[file][libName],
        `Libraries missing for deploying transaction ${txIndex}`
      );

      const libTxId = libs[file][libName];
      const address = txIndexToContract.get(libTxId);

      assert.isDefined(
        address,
        `Trying to link a library deployed in ${libTxId} for tx ${txIndex} but id doesn't exist`
      );

      for (const ref of references) {
        code = linkHexStringBytecode(
          code,
          address!.address.toString("hex"),
          ref.start
        );
      }
    }
  }

  assert.notInclude(
    code,
    "_",
    `Libraries missing for deploying transaction ${txIndex}`
  );

  return Buffer.from(code, "hex");
}

async function runDeploymentTransactionTest(
  txIndex: number,
  tx: DeploymentTransaction,
  provider: EdrProviderWrapper,
  compilerOutput: CompilerOutput,
  txIndexToContract: Map<number, DeployedContract>
): Promise<SolidityStackTrace | string> {
  const file = compilerOutput.contracts[tx.file];

  assert.isDefined(
    file,
    `File ${tx.file} from transaction ${txIndex} doesn't exist`
  );

  const contract = file[tx.contract];

  assert.isDefined(
    contract,
    `Contract ${tx.contract} from transaction ${txIndex} doesn't exist`
  );

  const deploymentBytecode = linkBytecode(
    txIndex,
    contract.evm.bytecode,
    tx.libraries,
    txIndexToContract
  );

  const params = encodeConstructorParams(
    contract.abi,
    tx.params !== undefined ? tx.params : []
  );

  const data = Buffer.concat([deploymentBytecode, params]);

  const trace = await traceTransaction(provider, {
    value: tx.value !== undefined ? BigInt(tx.value) : undefined,
    data,
    gas: tx.gas !== undefined ? BigInt(tx.gas) : undefined,
  });

  if (trace === undefined) {
    throw new Error(
      "deployment transactions should either deploy a contract or fail"
    );
  }

  return trace;
}

async function runCallTransactionTest(
  txIndex: number,
  tx: CallTransaction,
  provider: EdrProviderWrapper,
  compilerOutput: CompilerOutput,
  contract: DeployedContract
): Promise<SolidityStackTrace | undefined> {
  const compilerContract =
    compilerOutput.contracts[contract.file][contract.name];

  let data: Buffer;

  if (tx.data !== undefined) {
    data = Buffer.from(toBytes(tx.data));
  } else if (tx.function !== undefined) {
    data = encodeCall(
      compilerContract.abi,
      tx.function,
      tx.params !== undefined ? tx.params : []
    );
  } else {
    data = Buffer.from([]);
  }

  const trace = await traceTransaction(provider, {
    to: contract.address,
    value: tx.value !== undefined ? BigInt(tx.value) : undefined,
    data,
    gas: tx.gas !== undefined ? BigInt(tx.gas) : undefined,
  });

  if (typeof trace === "string") {
    throw new Error("call transactions should not deploy contracts");
  }

  return trace;
}

const onlyLatestSolcVersions =
  process.env.EDR_TESTS_ALL_SOLC_VERSIONS === undefined;

const filterSolcVersionBy =
  (versionRange: string) =>
  ({ solidityVersion, latestSolcVersion }: SolidityCompiler) => {
    if (onlyLatestSolcVersions && latestSolcVersion !== true) {
      return false;
    }

    return semver.satisfies(solidityVersion, versionRange);
  };

const solidity05Compilers = solidityCompilers.filter(
  filterSolcVersionBy("^0.5.0")
);
const solidity06Compilers = solidityCompilers.filter(
  filterSolcVersionBy("^0.6.0")
);
const solidity07Compilers = solidityCompilers.filter(
  filterSolcVersionBy("^0.7.0")
);
const solidity08Compilers = solidityCompilers.filter(
  filterSolcVersionBy("^0.8.0")
);

describe("Stack traces", function () {
  setCWD();

  // if a path to a solc file was specified, we only run these tests and use
  // that compiler
  const customSolcPath = process.env.EDR_TESTS_SOLC_PATH;
  if (customSolcPath !== undefined) {
    const customSolcVersion = process.env.EDR_TESTS_SOLC_VERSION;

    if (customSolcVersion === undefined) {
      console.error(
        "EDR_TESTS_SOLC_VERSION has to be set when using EDR_TESTS_SOLC_PATH"
      );
      process.exit(1);
    }

    if (!path.isAbsolute(customSolcPath)) {
      console.error("EDR_TESTS_SOLC_PATH has to be an absolute path");
      process.exit(1);
    }

    // eslint-disable-next-line mocha/no-exclusive-tests
    describe.only(`Use compiler at ${customSolcPath} with version ${customSolcVersion}`, function () {
      const compilerOptions = {
        solidityVersion: customSolcVersion,
        compilerPath: customSolcPath,
      };

      const testsDir = semver.satisfies(customSolcVersion, "^0.5.0")
        ? "0_5"
        : semver.satisfies(customSolcVersion, "^0.6.0")
          ? "0_6"
          : semver.satisfies(customSolcVersion, "^0.7.0")
            ? "0_7"
            : semver.satisfies(customSolcVersion, "^0.8.0")
              ? "0_8"
              : null;

      if (testsDir === null) {
        console.error(`There are no tests for version ${customSolcVersion}`);
        process.exit(1);
      }

      defineDirTests(
        path.join(__dirname, "test-files", testsDir),
        compilerOptions
      );

      defineDirTests(
        path.join(__dirname, "test-files", "version-independent"),
        compilerOptions
      );
    });

    return;
  }

  before("Download solc binaries", async function () {
    const solidityCompilersToDownload = [
      ...solidity05Compilers,
      ...solidity06Compilers,
      ...solidity07Compilers,
      ...solidity08Compilers,
    ];

    this.timeout(
      solidityCompilersToDownload.length * COMPILER_DOWNLOAD_TIMEOUT
    );

    for (const { solidityVersion } of solidityCompilersToDownload) {
      await downloadCompiler(solidityVersion);
    }
  });

  defineTestForSolidityMajorVersion(solidity05Compilers, "0_5");
  defineTestForSolidityMajorVersion(solidity06Compilers, "0_6");
  defineTestForSolidityMajorVersion(solidity07Compilers, "0_7");
  defineTestForSolidityMajorVersion(solidity08Compilers, "0_8");
});

describe("Solidity support", function () {
  it("check that the latest tested version is within the supported version range", async function () {
    const latestSupportedVersion = getLatestTestedSolcVersion();
    assert.isTrue(
      semver.satisfies(
        latestSupportedVersion,
        SUPPORTED_SOLIDITY_VERSION_RANGE
      ),
      `Expected ${latestSupportedVersion} to be within the ${SUPPORTED_SOLIDITY_VERSION_RANGE} range`
    );

    const nextPatchVersion = semver.inc(latestSupportedVersion, "patch")!;
    const nextMinorVersion = semver.inc(latestSupportedVersion, "minor")!;
    const nextMajorVersion = semver.inc(latestSupportedVersion, "major")!;

    assert.isFalse(
      semver.satisfies(nextPatchVersion, SUPPORTED_SOLIDITY_VERSION_RANGE),
      `Expected ${nextPatchVersion} to not be within the ${SUPPORTED_SOLIDITY_VERSION_RANGE} range`
    );
    assert.isFalse(
      semver.satisfies(nextMinorVersion, SUPPORTED_SOLIDITY_VERSION_RANGE),
      `Expected ${nextMinorVersion} to not be within the ${SUPPORTED_SOLIDITY_VERSION_RANGE} range`
    );
    assert.isFalse(
      semver.satisfies(nextMajorVersion, SUPPORTED_SOLIDITY_VERSION_RANGE),
      `Expected ${nextMajorVersion} to not be within the ${SUPPORTED_SOLIDITY_VERSION_RANGE} range`
    );
  });

  it("check that the latest tested version matches the one that EDR exports", async function () {
    const latestSupportedVersion = getLatestTestedSolcVersion();
    const edrLatestSupportedVersion = getLatestSupportedSolcVersion();

    assert.equal(latestSupportedVersion, edrLatestSupportedVersion);
  });
});

function defineTestForSolidityMajorVersion(
  solcVersionsCompilerOptions: SolidityCompiler[],
  testsPath: string
) {
  for (const compilerOptions of solcVersionsCompilerOptions) {
    // eslint-disable-next-line mocha/no-exclusive-tests
    const describeFn = compilerOptions.only === true ? describe.only : describe;

    describeFn(`Use compiler ${compilerOptions.compilerPath}`, function () {
      defineDirTests(
        path.join(__dirname, "test-files", testsPath),
        compilerOptions
      );

      defineDirTests(
        path.join(__dirname, "test-files", "version-independent"),
        compilerOptions
      );
    });
  }
}
