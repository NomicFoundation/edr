#!/usr/bin/env bash
#
# Publish a locally-built EDR NAPI package to a (Verdaccio) registry.
#
# This is the single source of truth for the "build EDR locally and publish it
# to Verdaccio" flow used by:
#   - book/src/02_development/03_local_release.md (manual local release)
#   - .claude/skills/setup-verdaccio-env/SKILL.md
#   - .github/workflows/edr-regression-benchmark.yml
#
# It stages the prebuilt native binary into its platform package, compiles the
# bundled TypeScript helpers, pins both the main and platform packages to the
# requested version, wires the main package's dependency on the platform
# package, and publishes both (platform package first).
#
# It deliberately does NOT run scripts/prepublish.sh: that injects all 7 platform
# packages as hard dependencies, but a local build only produces (and publishes)
# the current platform. We wire the single platform dependency instead — which is
# all `crates/edr_napi/index.js` needs to load the binding on this platform.
#
# The native binary must already be built (e.g. `pnpm build` in crates/edr_napi)
# and EDR's dependencies installed (the TypeScript compile needs the local `tsc`).
#
# NOTE: This mutates tracked files in crates/edr_napi (package.json versions, the
# wired dependency, the staged .node binary, coverage.sol and dist/). Reset them
# with `git checkout -- crates/edr_napi` afterwards.
#
# Usage:
#   scripts/publish_to_verdaccio.sh --version <ver> [options]
#
# Options:
#   --version <ver>     Version to publish, e.g. 0.12.1-local.abcdef (required)
#   --registry <url>    Target registry (default: http://127.0.0.1:4873/)
#   --npmrc <path>      npmrc with the registry auth token (sets
#                       NPM_CONFIG_USERCONFIG); omit if already logged in
#   -h, --help          Show this help
#
# The platform package (e.g. linux-x64-gnu) is autodetected from this host.

set -euo pipefail

REGISTRY="http://127.0.0.1:4873/"
VERSION=""
NPMRC=""

usage() {
  sed -n '2,/^set -euo/p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//; $d'
}

while [ $# -gt 0 ]; do
  case "$1" in
    --version)
      [ $# -ge 2 ] || { echo "error: --version requires a value" >&2; usage >&2; exit 1; }
      VERSION="$2"; shift 2 ;;
    --registry)
      [ $# -ge 2 ] || { echo "error: --registry requires a value" >&2; usage >&2; exit 1; }
      REGISTRY="$2"; shift 2 ;;
    --npmrc)
      [ $# -ge 2 ] || { echo "error: --npmrc requires a value" >&2; usage >&2; exit 1; }
      NPMRC="$2"; shift 2 ;;
    -h|--help) usage; exit 0 ;;
    *) echo "error: unknown argument: $1" >&2; usage >&2; exit 1 ;;
  esac
done

if [ -z "$VERSION" ]; then
  echo "error: --version is required" >&2
  usage >&2
  exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
NAPI_DIR="$REPO_ROOT/crates/edr_napi"

# Detect the platform package suffix (e.g. linux-x64-gnu) from this host.
PLATFORM="$(node "$SCRIPT_DIR/detect_edr_platform.cjs")"

PLATFORM_DIR="$NAPI_DIR/npm/$PLATFORM"
BINARY="edr.$PLATFORM.node"

if [ ! -d "$PLATFORM_DIR" ]; then
  echo "error: no platform package for '$PLATFORM' (expected $PLATFORM_DIR)" >&2
  echo "       known platforms: $(cd "$NAPI_DIR/npm" && echo */ | tr -d '/')" >&2
  exit 1
fi

echo ">> Publishing @nomicfoundation/edr@$VERSION ($PLATFORM) to $REGISTRY"

if [ ! -f "$NAPI_DIR/$BINARY" ]; then
  echo "error: native binary $BINARY not found in $NAPI_DIR" >&2
  echo "       build it first (e.g. 'pnpm build' in crates/edr_napi)" >&2
  exit 1
fi

echo ">> Staging $BINARY into the platform package"
cp "$NAPI_DIR/$BINARY" "$PLATFORM_DIR/$BINARY"

# The published package's `files`/`exports` reference coverage.sol and dist/src
# (the `@nomicfoundation/edr/coverage` export), so produce them.
echo ">> Compiling bundled TypeScript helpers"
cp "$REPO_ROOT/data/contracts/coverage.sol" "$NAPI_DIR/coverage.sol"
( cd "$NAPI_DIR" && pnpm exec tsc )

echo ">> Pinning versions and wiring the platform dependency"
( cd "$NAPI_DIR"
  npm pkg set version="$VERSION"
  npm pkg set version="$VERSION" --prefix "npm/$PLATFORM"
  npm pkg set "dependencies.@nomicfoundation/edr-$PLATFORM=$VERSION"
)

if [ -n "$NPMRC" ]; then
  export NPM_CONFIG_USERCONFIG="$NPMRC"
fi

# Publish the platform package first so the main package's dependency resolves.
echo ">> Publishing @nomicfoundation/edr-$PLATFORM@$VERSION"
( cd "$PLATFORM_DIR" && pnpm publish --registry="$REGISTRY" --no-git-checks --access public )

echo ">> Publishing @nomicfoundation/edr@$VERSION"
( cd "$NAPI_DIR" && pnpm publish --registry="$REGISTRY" --no-git-checks --access public )

echo ">> Done. Published @nomicfoundation/edr@$VERSION (+ -$PLATFORM) to $REGISTRY"
