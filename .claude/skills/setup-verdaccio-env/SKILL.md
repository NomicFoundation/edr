---
name: setup-verdaccio-env
description: Set up a local Verdaccio environment to test EDR + Hardhat 3 + a third-party repo (e.g. openzeppelin-contracts). Builds EDR with a chosen Rust profile, publishes EDR and Hardhat packages to a local Verdaccio registry, and installs them in the target repo. Use when you need to test local EDR changes in a realistic npm-installed setup.
disable-model-invocation: true
argument-hint: <target-repo-path> [--edr-build-script <build:dev|build:perf-js|build:debug|...>]
---

## Overview

This skill sets up a local Verdaccio npm registry and publishes locally-built EDR and Hardhat packages to it, then installs them in a third-party target repo. This avoids situations where `pnpm link` is not sufficient and ensures the target repo uses the packages exactly as they would be installed from npm.

## Arguments

- `$1` (required): Absolute path to the target repo (e.g. `/workspaces/openzeppelin-contracts`)
- `--edr-build-script` (optional): The `pnpm run` script name to build EDR napi. Defaults to `build:perf-js`.

If no arguments are provided, ask the user for the target repo path and which build profile to use.

## Constraints

- The Hardhat repo is expected at `/workspaces/hardhat`.
- The EDR repo is this repo (`/workspaces/edr`).
- The target repo must have `hardhat` in its `package.json`.

If the Hardhat or EDR directories cannot be found, ask the user to provide the correct paths.

## Steps

### 1. Build EDR

Build the napi binary for the current platform:

```bash
cd /workspaces/edr/crates/edr_napi
pnpm run <edr-build-script>
```

### 2. Start Verdaccio

The Hardhat repo has a built-in Verdaccio management script.

Before starting verdaccio, verify the `max_body_size` setting in `/workspaces/hardhat/.verdaccio/config.yaml`. EDR native binaries can be 60+ MB. If `max_body_size` is missing or less than 100mb:

1. If necessary, edit `/workspaces/hardhat/scripts/verdaccio/start.ts` to include `max_body_size: 100mb` in the config template (before the `log:` line).
2. Start verdaccio:
   ```bash
   cd /workspaces/hardhat
   pnpm verdaccio start --background
   ```

### 3. Publish the local EDR build to Verdaccio

Use the shared helper script. It autodetects the platform triple, stages the built binary into its platform package, compiles the bundled TypeScript helpers, pins both packages to the chosen version, wires the platform dependency, and publishes both (platform package first):

```bash
cd /workspaces/edr
scripts/publish_to_verdaccio.sh \
  --version <version> \
  --registry http://127.0.0.1:4873/ \
  --npmrc /workspaces/hardhat/.verdaccio/.npmrc
```

Choose a `<version>` that does not already exist on npm — bump the prerelease number from `crates/edr_napi/package.json` (e.g. `0.12.0-next.29` -> `0.12.0-next.30`). Verdaccio proxies `@nomicfoundation/*` to npmjs, so re-publishing an existing version fails. Note the chosen `<version>`; the Hardhat EDR dependency update in the next step must match it.

### 4. Bump and publish Hardhat packages to Verdaccio

First, update Hardhat's EDR dependency to the new version:

- Edit `/workspaces/hardhat/v-next/hardhat/package.json`: update `@nomicfoundation/edr`

Then, identify ALL Hardhat workspace packages that the target repo depends on. Parse the target repo's `package.json` for `@nomicfoundation/*` and `hardhat` deps.

For each such package, check if it has commits since its last npm release:

```bash
cd /workspaces/hardhat
git log --oneline '<package-name>@<current-version>'..HEAD -- v-next/<package-dir>/
```

For packages with changes: bump their patch version in their `package.json`.

After all bumps, reinstall Hardhat deps:

```bash
cd /workspaces/hardhat
pnpm install --no-frozen-lockfile
```

Also bump the `hardhat` package version itself (patch bump).

Publish all bumped packages to Verdaccio. For each bumped package:

```bash
cd /workspaces/hardhat/v-next/<package>
NPM_CONFIG_USERCONFIG=/workspaces/hardhat/.verdaccio/.npmrc \
  pnpm publish --no-git-checks --access public --registry http://127.0.0.1:4873/
```

Publish in dependency order: `hardhat-errors` and `hardhat-utils` first (they are commonly depended upon), then the rest, then `hardhat` itself last.

### 5. Install in the target repo

Detect the target repo's package manager by checking which lockfile exists:

| Lockfile | Package manager | Install command |
| --- | --- | --- |
| `pnpm-lock.yaml` | pnpm | `pnpm install --no-frozen-lockfile --registry http://127.0.0.1:4873/` |
| `yarn.lock` | yarn | `yarn install --registry http://127.0.0.1:4873/` |
| `package-lock.json` | npm | `npm install --registry http://127.0.0.1:4873/` |
| (none) | npm (default) | `npm install --registry http://127.0.0.1:4873/` |

Update the target repo's `package.json` to require the new versions. For each bumped package, update the version range (e.g. `^3.2.1` for hardhat, `^0.12.0-next.30` for edr).

Then clean install:

```bash
cd <target-repo>
rm -rf node_modules
<detected-install-command>
```

### 6. Validate

`<suffix>` below is the platform triple the publish script reported in step 3 (e.g. `linux-x64-gnu`, `darwin-arm64`) — the subdirectory of `crates/edr_napi/npm/` matching this host.

Verify all installed versions match what was published:

```bash
for pkg in hardhat @nomicfoundation/edr @nomicfoundation/edr-<suffix> <other-deps...>; do
  version=$(grep '"version"' <target-repo>/node_modules/$pkg/package.json | head -1)
  echo "$pkg: $version"
done
```

Also verify the native binary is the correct one:

```bash
ls -la <target-repo>/node_modules/@nomicfoundation/edr-<suffix>/edr.<suffix>.node
nm <target-repo>/node_modules/@nomicfoundation/edr-<suffix>/edr.<suffix>.node 2>/dev/null | grep -c 'edr_provider'
```

Report the installed versions to the user and confirm the environment is ready.

### 7. Cleanup reminder

Remind the user that:

- Verdaccio is still running in the background. Stop it with: `cd /workspaces/hardhat && pnpm verdaccio stop`
- The version bumps in EDR and Hardhat repos are local only. Reset with: `cd /workspaces/edr && git checkout -- crates/edr_napi/package.json crates/edr_napi/npm/` `cd /workspaces/hardhat && git checkout -- v-next/`
- The target repo's `package.json` and lockfile have been modified. Reset with: `cd <target-repo> && git checkout -- package.json <lockfile>`
