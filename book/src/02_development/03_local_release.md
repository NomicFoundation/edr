# Local release

These are instructions for releasing the [EDR NPM package](../../crates/edr_napi/package.json) locally for debugging purposes.

1. Install and start [Verdaccio](./02_verdaccio.md), and log in (`pnpm login --registry=http://localhost:4873/`).
2. Build the NAPI package for your platform: run `pnpm build` in the [edr_napi](../../crates/edr_napi/) directory (or `pnpm build:dev` for a faster, unoptimized build).
3. Publish the local build with the helper script, which stages the native binary into its platform package, pins the version, wires the platform dependency, and publishes both packages:

   ```bash
   scripts/publish_to_verdaccio.sh --version <version> --registry http://localhost:4873/
   ```

   Pick a `--version` that differs from any version already on npm (e.g. `0.12.1-local.1`), because Verdaccio proxies `@nomicfoundation/*` to npmjs and will reject re-publishing an existing version. The platform package is autodetected; pass `--help` for all options.

> The script mutates tracked files under `crates/edr_napi/`. Reset them afterwards with `git checkout -- crates/edr_napi`.
