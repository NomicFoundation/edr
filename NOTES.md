# Static hardfork module (`@nomicfoundation/edr/hardforks`)

Personal note to resume later. Full detail in `PLAN-static-hardfork-module.md`.

## Goal
Let Hardhat (and others) read hardfork metadata — `SpecId`/`OpHardfork` enums, name
strings, conversions, latest — **without loading the native `.node`**, with the
source in EDR (so HH can drop its hand-maintained mirror).

## What's implemented (branch state)
- **`src/ts/hardforks.ts`** — hand-maintained, self-contained (imports nothing).
  Holds the enums, name consts, `L1_HARDFORK_LATEST`/`OP_HARDFORK_LATEST`, and the
  conversions (one `*_HARDFORK_TO_NAME` map + derived inverse; shared generic
  helpers across L1/OP). `l1HardforkLatest()`/`opLatestHardfork()` kept but
  `@deprecated` → point to the constants.
- **`src/ts/index.ts`** — the public `.` entry (a wrapper): `export * from "#native"`
  + `export * from "./hardforks.js"`, and re-types `l1GenesisState`/`opGenesisState`
  back to `SpecId`/`OpHardfork` (converting to a name string before the native call).
- **napi boundary** — genesis takes a hardfork **name `string`**; the `SpecId`/
  `OpHardfork` enums + name consts + conversions were **removed from `#[napi]`** (now
  only in `hardforks.ts`). Rust has hand-written `l1_hardfork_from_name` /
  `op_hardfork_from_name` matches in `l1.rs`/`op.rs`.
- **`package.json`** — `.`→`dist/src/ts/index.js`, `./hardforks`→`dist/src/ts/hardforks.js`,
  `imports: { "#native": "./index.js" }`. Build is two-pass `tsc` (`tsconfig.build.json`
  then `tsc --noEmit`) in `scripts/build_edr_napi.sh`.

## Key decisions
- **String boundary** at the addon (numeric enum values never cross FFI); the wrapper
  restores the enum-typed public signatures.
- **Zero breaking changes**: genesis signatures preserved (via wrapper); both `*HardforkLatest` deprecated toward the constants.

## Design note: the slim `.` wrapper
napi-rs owns `index.js`/`index.d.ts` (the native loader) — it's regenerated on every
build, can't be hand-edited, and *importing it loads the `.node`*. We need the `.`
entry to also surface the addon-free `hardforks.ts` and to re-type genesis. So `.` is a
hand-written wrapper (`src/ts/index.ts`), and the napi-generated file becomes an
*internal* loader reached via the package `imports` subpath `#native`.

- `export * from "#native"` (the napi bindings) + `export * from "./hardforks.js"` (the
  static metadata) → both importable from `@nomicfoundation/edr`, same types as `/hardforks`.
- The wrapper also declares `l1GenesisState`/`opGenesisState` locally (typed as
  `SpecId`/`OpHardfork`, forwarding to the native string-typed fns). A **local export
  takes precedence over the same name from `export *`**, so consumers get the enum-typed
  signatures while the addon boundary stays string-based.
- `#native` is used instead of a relative path because (a) napi can't rename the
  `--platform` loader, and (b) the subpath is location-independent, so it resolves the
  same from source and from `dist/` (a plain `../../index` would not). This is also why
  the build is two-pass: pass 1 emits the wrapper's `.d.ts` so the tests' `import … from ".."`
  resolves to it; pass 2 type-checks.

## ⚠️ Main open issue (resume here)
We **moved the authoritative source of hardfork name strings** from "revm name modules
(referenced everywhere)" to **hand-typed literals in two EDR places** (`hardforks.ts`
*and* the Rust converters). They match revm today but are independent copies with **no
drift guard**.

But the provider config + base-fee config still parse the hardfork string via revm's
`Hardfork::FromStr` — so the names are **still functionally bound to revm**; they can't
diverge without breaking provider config. The decoupling is **structural only** right now.


## Follow-ups
- **Auto-generate `hardforks.ts`**: extract the hardfork metadata into a **napi-free crate** 
  that a generator binary can depend on
  (since `edr_napi` can't be linked into a bin), with a `--check` mode for CI — like the
  repo's `edr_tool_op_chain_config_generator`.
- **Hardhat PR**: import `SpecId`/names/`L1_HARDFORK_LATEST`/conversions from
  `@nomicfoundation/edr/hardforks`; delete HH's mirror; `getCurrentHardfork` reads
  `L1_HARDFORK_LATEST`.
- **Optional**: benchmark `edr.node` load time to quantify the payoff (never measured).

## Validation status
- `cargo check -p edr_napi --features op` clean; full `pnpm test` green **except** one
  pre-existing non-TTY chalk color flake in `logs.ts` (unrelated; passes with `FORCE_COLOR=1`).
- Verified the port is lossless: every symbol removed from the napi bindings is present in
  `hardforks.ts` (enum members + values, all name consts, all conversions), and the name
  string **values are byte-identical** to the `revm`/`op-revm` name modules `main` used.

