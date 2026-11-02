import {
  Compiler as SolcJsCompiler,
  NativeCompiler,
} from "hardhat/internal/solidity/compiler";
import {
  Compiler,
  CompilerDownloader,
  CompilerPlatform,
} from "hardhat/internal/solidity/compiler/downloader";
import { getCompilersDir } from "hardhat/internal/util/global-dir";

import { CompilerInput, CompilerOutput } from "hardhat/types";

interface SolidityCompilerOptimizer {
  viaIR: boolean;
  runs: number;
}

interface SolidityCompiler {
  solidityVersion: string;
  compilerPath: string;
  optimizer?: SolidityCompilerOptimizer;
}

interface SolcSourceFileToContents {
  [filename: string]: { content: string };
}

function getSolcInput(
  sources: SolcSourceFileToContents,
  compilerOptions: SolidityCompiler
): CompilerInput {
  const isViaIR = compilerOptions.optimizer?.viaIR ?? false;

  const optimizerDetails = isViaIR
    ? {
        yulDetails: {
          optimizerSteps: "u",
        },
      }
    : undefined;

  const optimizer =
    compilerOptions.optimizer === undefined
      ? {
          enabled: false,
        }
      : {
          enabled: true,
          runs: compilerOptions.optimizer.runs,
          details: optimizerDetails,
        };

  const settings: CompilerInput["settings"] = {
    optimizer,
    outputSelection: {
      "*": {
        "*": [
          "abi",
          "evm.bytecode",
          "evm.deployedBytecode",
          "evm.methodIdentifiers",
        ],
        "": ["id", "ast"],
      },
    },
  };

  // old compilers might throw if we use the `viaIR` setting because they don't
  // recognize the option, so we only set it when it's true
  if (isViaIR) {
    settings.viaIR = true;
  }

  return {
    language: "Solidity",
    sources,
    settings,
  };
}

function getSolcInputForLiteral(
  source: string,
  compilerOptions: SolidityCompiler,
  filename: string = "literal.sol"
): CompilerInput {
  return getSolcInput({ [filename]: { content: source } }, compilerOptions);
}

async function compile(
  input: CompilerInput,
  compiler: Compiler
): Promise<[CompilerInput, CompilerOutput]> {
  let runnableCompiler: any;
  if (compiler.isSolcJs) {
    runnableCompiler = new SolcJsCompiler(compiler.compilerPath);
  } else {
    runnableCompiler = new NativeCompiler(compiler.compilerPath);
  }

  const output = await runnableCompiler.compile(input);

  if (output.errors !== undefined) {
    for (const error of output.errors) {
      if (error.severity === "error") {
        throw new Error(`Failed to compile: ${error.message}`);
      }
    }
  }

  return [input, output];
}

export async function compileLiteral(
  source: string,
  compilerOptions: SolidityCompiler = {
    solidityVersion: "0.8.0",
    compilerPath: "soljson-v0.8.0+commit.c7dfd78e.js",
  },
  filename: string = "literal.sol"
): Promise<[CompilerInput, CompilerOutput]> {
  await downloadCompiler(compilerOptions.solidityVersion);
  const compiler = await getCompilerForVersion(compilerOptions.solidityVersion);

  return compile(
    getSolcInputForLiteral(source, compilerOptions, filename),
    compiler
  );
}

async function getCompilerForVersion(
  solidityVersion: string
): Promise<Compiler> {
  const compilersCache = await getCompilersDir();
  const downloader = CompilerDownloader.getConcurrencySafeDownloader(
    CompilerDownloader.getCompilerPlatform(),
    compilersCache
  );

  const compiler = await downloader.getCompiler(solidityVersion);
  if (compiler !== undefined) {
    return compiler;
  }

  const wasmDownloader = CompilerDownloader.getConcurrencySafeDownloader(
    CompilerPlatform.WASM,
    compilersCache
  );

  const wasmCompiler = await wasmDownloader.getCompiler(solidityVersion);

  if (wasmCompiler === undefined) {
    throw new Error("Expected compiler to be downloaded");
  }

  return wasmCompiler;
}

export async function downloadCompiler(solidityVersion: string): Promise<void> {
  const compilersCache = await getCompilersDir();
  const downloader = CompilerDownloader.getConcurrencySafeDownloader(
    CompilerDownloader.getCompilerPlatform(),
    compilersCache
  );

  const isCompilerDownloaded =
    await downloader.isCompilerDownloaded(solidityVersion);

  if (!isCompilerDownloaded) {
    console.log("Downloading solc", solidityVersion);
    await downloader.downloadCompiler(
      solidityVersion,
      async () => {},
      async () => {}
    );
  }

  const compiler = await downloader.getCompiler(solidityVersion);

  if (compiler !== undefined) {
    return;
  }

  const wasmDownloader = CompilerDownloader.getConcurrencySafeDownloader(
    CompilerPlatform.WASM,
    compilersCache
  );

  const isWasmCompilerDownloaded =
    await downloader.isCompilerDownloaded(solidityVersion);

  if (!isWasmCompilerDownloaded) {
    console.log("Downloading solcjs", solidityVersion);
    await wasmDownloader.downloadCompiler(
      solidityVersion,
      async () => {},
      async () => {}
    );
  }
}
