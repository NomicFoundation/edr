#!/usr/bin/env bash

set -e
set -x
set -o pipefail

# `napi pre-publish` defaults to running `${npmClient} publish` for each
# platform package (gated only by `existsSync` on the staged .node) and to
# calling `octokit.repos.createRelease` (gated only by GITHUB_TOKEN being set
# in the env). Both flags below make the safety explicit rather than relying
# on call-site ordering and env composition:
#   --skip-optional-publish  — don't run `npm publish` for the platform pkgs.
#   --no-gh-release          — don't try to create a GitHub release.
pnpm napi pre-publish -t npm --skip-optional-publish --no-gh-release

jq 'with_entries(if .key == "optionalDependencies" then .key = "dependencies" else . end)' package.json | sponge package.json
