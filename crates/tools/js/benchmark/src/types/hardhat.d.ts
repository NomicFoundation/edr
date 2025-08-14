declare module "hardhat/internal/builtin-plugins/solidity-test/helpers" {
  export function solidityTestConfigToSolidityTestRunnerConfigArgs(
    chainType: ChainType,
    projectRoot: string,
    config: SolidityTestConfig,
    verbosity: number,
    observability?: ObservabilityConfig,
    testPattern?: string
  ): SolidityTestRunnerConfigArgs;
}
