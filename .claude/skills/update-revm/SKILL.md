---
name: update-revm
description: Update REVM and related crate dependencies (revm, revm-primitives, revm-interpreter,  revm-precompile, revm-handler, revm-context, revm-context-interface, revm-database-interface, revm-state, revm-bytecode, revm-inspector, op-revm) to a specific release version. Fetches version information from the REVM GitHub repository at a git tag, updates all Cargo.toml workspace dependencies, fixes compilation errors, and runs quality checks.
disable-model-invocation: true
context: fork
agent: general-purpose
argument-hint: <revm-git-tag> (e.g., v103)
---

# Update REVM Dependencies

You are updating the REVM dependencies in this Rust workspace to the versions at git tag `$ARGUMENTS`.

## Step 1: Discover target versions

Fetch the REVM workspace root Cargo.toml at the tag to discover workspace members:

```
https://raw.githubusercontent.com/bluealloy/revm/$ARGUMENTS/Cargo.toml
```

Parse `[workspace].members` to find all crates. Then for each crate this workspace uses, fetch its Cargo.toml to get `[package].version`:

```
https://raw.githubusercontent.com/bluealloy/revm/$ARGUMENTS/crates/<path>/Cargo.toml
```

Known crate paths (verify against workspace members):

- `crates/revm` → revm
- `crates/primitives` → revm-primitives
- `crates/interpreter` → revm-interpreter
- `crates/precompile` → revm-precompile
- `crates/handler` → revm-handler
- `crates/context` → revm-context
- `crates/context/interface` → revm-context-interface
- `crates/database` → revm-database
- `crates/database/interface` → revm-database-interface
- `crates/state` → revm-state
- `crates/bytecode` → revm-bytecode
- `crates/inspector` → revm-inspector
- `crates/op` → op-revm

Build a complete mapping: `crate-name → version`.

## Step 2: Update dependency versions

Read the root `Cargo.toml` and update every REVM crate version in `[workspace.dependencies]`. Also search all individual crate `Cargo.toml` files for REVM dependencies that specify their own version (i.e., not using `workspace = true`) and update those too. **Keep all existing features, default-features, and other settings unchanged** — only update version numbers.

Also check and update these related ecosystem crates if needed:

- `revm-inspectors` (from paradigmxyz/revm-inspectors) — find a compatible version/commit
- `foundry-fork-db` — find a compatible version if it exists
- `c-kzg` — update if the REVM release requires a newer version

If the project uses a `[patch]` section for any of these crates, note it may need updating.

## Step 3: Compile and fix errors

Run `cargo clippy --all-targets --all-features --workspace 2>&1` and iteratively fix compilation errors.

First, fetch the REVM migration guide for guidance on breaking API changes:

```
https://raw.githubusercontent.com/bluealloy/revm/main/MIGRATION_GUIDE.md
```

### Foundry upstream reference

This project vendors Foundry code under `crates/foundry/`. Use the upstream Foundry PR for the same REVM upgrade as a reference for fixing compilation errors in our Foundry-derived code.

- Repo: https://github.com/foundry-rs/foundry
- Example PR for REVM 34: https://github.com/foundry-rs/foundry/pull/13130

To find the relevant PR for the target REVM version, search the Foundry repo for PRs mentioning the REVM version (e.g., `gh search prs --repo foundry-rs/foundry "revm 34" --state merged`).

Use `gh pr diff <number> --repo foundry-rs/foundry` to fetch the diff for reference. Focus on changes to files that correspond to code under our `crates/foundry/` directory. Apply analogous fixes, adapting for any local differences in our vendored code.

Common REVM upgrade issues:

- Renamed types, traits, or methods
- Changed function signatures or generic parameters
- Items moved between modules
- New required trait implementations
- Changed error types or enums with new variants

For each error:

1. Read the error carefully
2. Consult the migration guide for known breaking changes and their fixes
3. Look up the relevant REVM source at the target tag if needed: `https://raw.githubusercontent.com/bluealloy/revm/$ARGUMENTS/crates/<crate>/src/<file>`
4. Apply the minimal fix
5. Re-check compilation

Repeat until `cargo clippy --all-targets --all-features --workspace` succeeds.

## Step 4: Quality checks

Run each check and fix issues before moving to the next:

1. **Formatting**: `cargo +nightly fmt --check 2>&1`

   - If it fails, run `cargo +nightly fmt` to auto-format

2. **Documentation**: `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps 2>&1`

   - Fix any doc warnings

3. **Tests**: `cargo test --all-targets --all-features --workspace 2>&1`
   - Fix any failing tests due to API changes

## Step 5: Summary

Report:

- Version changes (old → new) for each crate
- Code changes required by API differences (files modified and why)
- Quality check results (pass/fail for each)
- Any remaining issues or TODOs
