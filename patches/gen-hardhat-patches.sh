#!/bin/bash

# This script is used to conveniently generate patches for Hardhat to be used with `pnpm patch`
# We have to do this because of the way Hardhat is structured; it uses a specific
# monorepo setup, so we attempt to cherry-pick the outlined changes and compile
# the resulting .js files ourselves to be used with `pnpm patch`.

script_dir="$(cd "$(dirname "$0")" && pwd)"

temp_dir=$(mktemp -d 2>/dev/null || mktemp -d -t 'mytempdir')

trap "rm -rf '$temp_dir'" EXIT

pnpm patch hardhat --edit-dir $temp_dir

cd $temp_dir

COMMITS=(
  # Adapt to the EDR v0.5 breaking changes (drop when Hardhat v2.22.7 releases)
  f944cd54eb65d78f6ce54248fb5c573aadc7281c
  # refactor: Remove dead code and hide unnecessarily public properties
  5739893bf382b4e937b44995ca7917cbbd39de12
  # refactor: Re-use the compiler to model and opcodes logic from EDR
  49c66b6947283d9c9414f5e6faf2f94dcf05cc58
  # refactor: Re-use MessageTrace and VmTracer from EDR now
  3aeeb564394349824221ea9814f49cb5f8002d78
)

for commit in "${COMMITS[@]}"; do
 # We're only interested in the main `hardhat-core` package:
 # strip the /packages/hardhat-core/ prefix when applying the patch
 # Also ignore rejectfiles to minimize the resulting patch; assume the user knows what they're doing
curl "https://github.com/NomicFoundation/hardhat/commit/$commit.patch" | patch --strip 3 --force --reject-file -
done

# Before adapting the package to build locally, back up the package.json
cp package.json package.json.bak

# Remove the local dev dependencies from the package.json that start with
# @nomicfoundation/eslint-plugin- as these are not available nor needed
jq '
  .devDependencies |= with_entries(
    select(.key | startswith("@nomicfoundation/eslint-plugin-") | not)
  )
' package.json > package.json.tmp && mv package.json.tmp package.json

# Recreate the tsconfig.json setup as it's pruned when packaged for publishing:
# First, the base monorepo tsconfig.json:
curl https://raw.githubusercontent.com/NomicFoundation/hardhat/main/config/typescript/tsconfig.json > tsconfig.base.json
# Then, the hardhat-core base tsconfig.json: (we're only interested in the references, not the test setup)
curl https://raw.githubusercontent.com/NomicFoundation/hardhat/main/packages/hardhat-core/tsconfig.json > tsconfig.json
jq '.extends = "./tsconfig.base.json"' tsconfig.json > tsconfig.json.tmp && mv tsconfig.json.tmp tsconfig.json
# echo '{"extends": "./tsconfig.base.json", "references": [{ "path": "./src" }] }' > tsconfig.json
# Finally, the tsconfig.json for the src directory:
curl https://raw.githubusercontent.com/NomicFoundation/hardhat/main/packages/hardhat-core/src/tsconfig.json > src/tsconfig.json
jq '.extends = "../tsconfig.base.json"' src/tsconfig.json > src/tsconfig.json.tmp && mv src/tsconfig.json.tmp src/tsconfig.json

# Finally, point the local @nomicfoundation/edr package to the local one:
jq ".devDependencies[\"@nomicfoundation/edr\"] = \"file:$script_dir/../crates/edr_napi\"" package.json > package.json.tmp && mv package.json.tmp package.json
jq ".dependencies[\"@nomicfoundation/edr\"] = \"file:$script_dir/../crates/edr_napi\"" package.json > package.json.tmp && mv package.json.tmp package.json

pnpm install
pnpm build

# Restore the original package.json and remove aux files to minimize the resulting patch
mv package.json.bak package.json
rm tsconfig.base.json tsconfig.json src/tsconfig.json

cd -
pnpm patch-commit "$temp_dir"
