declare module "hardhat/internal/builtin-plugins/solidity-test/edr-artifacts" {
  export async function getBuildInfos(
    artifactManager: ArtifactManager
  ): Promise<BuildInfoAndOutput[]>;

  export async function getEdrArtifacts(
    artifactManager: ArtifactManager
  ): Promise<Array<{ edrAtifact: EdrArtifact; userSourceName: string }>>;
}

declare module "hardhat/internal/builtin-plugins/solidity/build-results" {
  type SolidityBuildResults =
    | Map<string, FileBuildResult>
    | CompilationJobCreationError;
  type SuccessfulSolidityBuildResults = Map<
    string,
    Exclude<FileBuildResult, FailedFileBuildResult>
  >;

  export function throwIfSolidityBuildFailed(
    results: SolidityBuildResults
  ): asserts results is SuccessfulSolidityBuildResults;
}
