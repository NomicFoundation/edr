// Type shim for `@nomicfoundation/hardhat-slang-solx`. The published plugin
// needs hardhat ^3.15.0 and this workspace pins hardhat 3.4.5, so consumers
// (including CI) don't install it. The sweep skips at runtime when the import
// fails (see `scripts/maybe-build.js` and `test/sweep.ts`), but `tsc` still
// type-checks `hardhat.config.ts`'s static import. This declaration keeps
// that compilation step happy.
//
// Delete this file once the workspace's hardhat satisfies the plugin's peer
// range and `@nomicfoundation/hardhat-slang-solx` becomes a regular
// `devDependencies` entry.
declare module "@nomicfoundation/hardhat-slang-solx" {
  const plugin: unknown;
  export default plugin;
}
