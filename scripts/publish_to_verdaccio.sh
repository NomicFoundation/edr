#!/usr/bin/env bash
#
# Publish a locally-built EDR NAPI package to a (Verdaccio) registry.
#
# It stages the prebuilt native binary into its platform package, compiles the
# bundled TypeScript helpers, pins the packages to the requested version, wires
# this host's platform package in as an optionalDependency — mirroring a real
# release's package shape — and publishes the main package and this platform's
# package (platform package first).
#
# A real release lists all platform packages; a local build only produces one,
# so only this host's is wired into `optionalDependencies`. npm, pnpm and
# bun skip `optionalDependencies` they can't resolve, but Yarn Classic
# ("Couldn't find any versions") and Yarn Berry (YN0082) both treat an
# unresolvable version as a fatal error.
#
# The native binary must already be built (e.g. `pnpm build` in crates/edr_napi)
# and EDR's dependencies installed (the TypeScript compile needs the local `tsc`).
#
# NOTE: This mutates tracked files in crates/edr_napi (package.json versions, the
# injected optionalDependencies, the staged .node binary, coverage.sol and
# dist/). Reset them with `git checkout -- crates/edr_napi` afterwards.
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

# @napi-rs/cli >= 3.8 validates during pre-publish that every target in
# `napi.targets` has its .node binary staged, and `--skip-optional-publish` does
# not exempt them — but only this host's binary was built. There is no flag to
# skip that validation, so hand `napi pre-publish` a config listing just this
# host's target: it then validates, versions and wires exactly the one platform
# package this script publishes, and never looks at the others.
#
# napi *merges* this file over package.json's `napi` field, so `binaryName`
# ("edr") is preserved and only `targets` is overridden. Its warning that the
# config file "will be used" reads as a replacement, but it is not one.
NAPI_CONFIG="$(mktemp)"
trap 'rm -f "$NAPI_CONFIG"' EXIT

node "$SCRIPT_DIR/write_napi_host_config.ts" "$NAPI_DIR" "$PLATFORM" "$NAPI_CONFIG"

if [ ! -s "$NAPI_CONFIG" ]; then
  echo "error: write_napi_host_config.ts produced no config" >&2
  exit 1
fi

echo ">> Pinning versions and wiring the platform package"
( cd "$NAPI_DIR"
  npm pkg set version="$VERSION"

  # Syncs the platform package's version and wires it as an optionalDependency,
  # mirroring the release workflow.
  "$SCRIPT_DIR/prepublish.sh" --config-path "$NAPI_CONFIG"
)

if [ -n "$NPMRC" ]; then
  export NPM_CONFIG_USERCONFIG="$NPMRC"
fi

# npm refuses to publish a prerelease (a version with a `-<pre-release>` segment,
# e.g. `0.12.1-local.<sha>`) to the implicit `latest` dist-tag, so publish those
# under an explicit tag. Consumers pin the exact version, so the tag name is only
# there to satisfy npm.
PUBLISH_ARGS=(--registry="$REGISTRY" --no-git-checks --access public)
case "$VERSION" in
  *-*) PUBLISH_ARGS+=(--tag local) ;;
esac

# Publish the platform package first so the main package's dependency resolves.
echo ">> Publishing @nomicfoundation/edr-$PLATFORM@$VERSION"
( cd "$PLATFORM_DIR" && pnpm publish "${PUBLISH_ARGS[@]}" )

echo ">> Publishing @nomicfoundation/edr@$VERSION"
( cd "$NAPI_DIR" && pnpm publish "${PUBLISH_ARGS[@]}" )

echo ">> Done. Published @nomicfoundation/edr@$VERSION (+ -$PLATFORM) to $REGISTRY"
