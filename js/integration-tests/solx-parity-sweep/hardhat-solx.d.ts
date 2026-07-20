// Type shim for `@nomicfoundation/hardhat-solx`. The plugin isn't published
// to npm yet, so consumers (including CI) don't actually install it. The
// sweep skips at runtime when the import fails (see `scripts/maybe-build.js`
// and `test/sweep.ts`), but `tsc` still type-checks `hardhat.config.ts`'s
// static import. This declaration keeps that compilation step happy.
//
// Delete this file once `@nomicfoundation/hardhat-solx` ships on npm and
// becomes a regular `devDependencies` entry.
declare module "@nomicfoundation/hardhat-solx" {
  const plugin: unknown;
  export default plugin;
}
