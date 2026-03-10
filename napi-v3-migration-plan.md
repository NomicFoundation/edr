# NAPI-RS v2 → v3 Migration Impact Analysis & Plan for `edr_napi`

## Context

The `edr_napi` crate uses NAPI-RS v2 (`napi 2.16.17`, `napi-derive 2.16.13`, `@napi-rs/cli ^2.18.4`) to expose Rust-based Ethereum development runtime functionality to Node.js across 7 platform targets. NAPI-RS v3 brings a redesigned ThreadsafeFunction API, moves legacy `Js*` types behind a `compat-mode` feature flag, and changes CLI flags and package.json config format.

All findings below have been validated against the actual v3 CLI (`@napi-rs/cli 3.5.1`), `docs.rs/napi/3.8.3`, the napi-rs GitHub source at `github.com/napi-rs/napi-rs/main/crates/napi/src/`, the official migration guide, and the v3 announcement.

---

## Key Questions

### 1. Do we need JS API modifications?

**No, the public TypeScript/JS API should remain unchanged.** The `#[napi(object)]` structs, `#[napi]` enums, and class method signatures produce identical TypeScript definitions in v3. The only risk area is callback parameters: currently typed with `#[napi(ts_type = "...")]` overrides on `JsFunction` fields. These overrides should be kept to ensure the generated signatures match the current API.

After migration, diff `index.d.ts` before/after to confirm. Expected: identical.

### 2. Can we build all currently supported platforms?

**Yes.** All 7 targets are fully supported by NAPI-RS v3, which expanded platform support to 14 targets:
- `x86_64-apple-darwin` / `aarch64-apple-darwin`
- `x86_64-unknown-linux-gnu` / `aarch64-unknown-linux-gnu`
- `x86_64-unknown-linux-musl` / `aarch64-unknown-linux-musl`
- `x86_64-pc-windows-msvc`

No platform-related issues expected.

Additionally, v3 introduces a `--cross-compile` (`-x`) flag for `napi build` that integrates with `cargo-zigbuild` and `cargo-xwin`. See [Future: CI Cross-Compilation Opportunity](#future-ci-cross-compilation-opportunity).

---

## What breaks: v2 → v3 incompatibilities

### Things that MUST change (won't compile)

These have no `compat-mode` shim — they break regardless of feature flags:

| What | Why | Scope |
|------|-----|-------|
| `ErrorStrategy` enum | **Removed entirely.** `ThreadsafeFunction<T, ErrorStrategy::Fatal>` won't compile. Replaced by `const CalleeHandled: bool` generic. | 6 files, ~10 type annotations |
| `ThreadsafeFunction` generic params | v2: `<T, ES>` (2 params). v3: `<T, Return, CallJsBackArgs, ErrorStatus, CalleeHandled, Weak, MaxQueueSize>` (7 params with defaults). | Same 6 files |
| `JsFunction::create_threadsafe_function` signature | v2: `(queue_size, callback)`. v3: `(callback)` only. Queue size is now a const generic. | 7 call sites |
| `call_with_return_value` callback signature | v2: `\|ret: T\| { ... }`. v3: `\|ret: Result<T>, env: Env\| { ... }`. | 4 call sites |
| `JsUnknown` type | Not available in v3, not even with `compat-mode`. Replaced by `Unknown<'_>`. | 1 file (`edr_napi_core/subscription.rs`) |
| `@napi-rs/cli` commands & flags | `prepublish` → `pre-publish`, `universal` → `universalize`, `--no-const-enum` removed, `--cargo-flags` → `--` separator | Build scripts, package.json |
| `package.json` napi config | `name` → `binaryName`, `triples` → `targets` (flat array) | package.json |

### Things behind `compat-mode` (can be deferred)

Validated against `napi-rs/napi-rs/main/crates/napi/src/js_values/mod.rs`:

| Type | `compat-mode` | Notes |
|------|:---:|---|
| `JsFunction` | ✅ | Used in 8 files as callback params/struct fields |
| `JsObject` | ✅ | Used in 3 files as deferred promise return types |
| `JsBoolean`, `JsBuffer`, `JsArrayBuffer`, `JsUndefined`, `JsNull` | ✅ | Minor usage |
| `JsString` / `JsStringUtf8` | Always available | Not behind any feature flag |

### Deprecated but still functional (no feature flag needed)

| API | Replacement | Status |
|-----|------------|--------|
| `Env::create_object()` | `Object::new(env)` | Deprecated since 3.0.0 |
| `Env::create_arraybuffer_with_data()` | `ArrayBuffer::from_data()` | Deprecated since 3.0.0 |
| `Env::create_buffer_with_data()` | `BufferSlice::from_data()` | Deprecated since 3.0.0 |
| `Env::create_string_from_std()` | Use native `String` | Deprecated since 3.0.0 |
| `Env::create_bigint_from_words()` | `BigInt` constructor | Deprecated since 3.0.0 |
| `Env::get_boolean()` | Use native `bool` | Deprecated since 3.0.0 |
| `Env::create_array_with_length()` | `Array` constructor | Deprecated since 3.0.0 |
| `ThreadsafeFunction::unref()` | `.weak::<true>()` builder | Deprecated since 2.17.0 |
| `ThreadsafeCallContext` / `ThreadSafeCallContext` | Builder pattern | Still available |

---

## What needs to change and where

### Infrastructure changes (all approaches)

These are the same regardless of migration strategy.

#### `crates/edr_napi/Cargo.toml` (and `edr_napi_core`)

| Dependency | v2 | v3 |
|---|---|---|
| `napi` | `"2.16.17"` | `"3"` (latest: 3.8.3) |
| `napi-derive` | `"2.16.13"` | `"3"` |
| `napi-build` | `"2.0.1"` | `"3"` |

Feature flags `napi8`, `async`, `error_anyhow`, `serde-json` — **all unchanged in v3**. Keep as-is.

#### `crates/edr_napi/package.json`

```jsonc
// BEFORE (v2):
"napi": {
  "name": "edr",
  "triples": { "defaults": false, "additional": [...] }
}
// AFTER (v3):
"napi": {
  "binaryName": "edr",
  "targets": [
    "aarch64-apple-darwin", "x86_64-apple-darwin",
    "aarch64-unknown-linux-gnu", "aarch64-unknown-linux-musl",
    "x86_64-unknown-linux-gnu", "x86_64-unknown-linux-musl",
    "x86_64-pc-windows-msvc"
  ]
}
```

- `@napi-rs/cli`: `"^2.18.4"` → `"^3"`
- Script `"universal": "napi universal"` → `"universalize"`
- Scripts `artifacts`, `version` — unchanged.

#### `scripts/build_edr_napi.sh`

```bash
# BEFORE:
napi build --platform --no-const-enum --cargo-flags="--locked" "$@"
# AFTER:
napi build --platform "$@" -- --locked
```

- Remove `--no-const-enum` (v3 defaults to no const enums; flag removed).
- `--cargo-flags="--locked"` → `-- --locked` (v3 uses `--` separator).
- `"$@"` before `--` because `--features`, `--profile` are now native `napi build` flags.

#### `scripts/prepublish.sh`

```bash
# BEFORE:
pnpm napi prepublish -t npm --skip-gh-release
# AFTER:
pnpm napi pre-publish -t npm -p npm
```

- `prepublish` → `pre-publish` (renamed).
- Remove `--skip-gh-release` (v3: GitHub release is opt-in via `--gh-release`).
- Add `-p npm` (explicit npm dir path).

#### `build.rs` — No changes needed

`napi_build::setup()` is unchanged in v3.

### Rust code changes

#### ThreadsafeFunction type annotations (6 files)

```rust
// v2:
use napi::threadsafe_function::{ErrorStrategy, ...};
let tsfn: ThreadsafeFunction<_, ErrorStrategy::Fatal> = ...;

// v3 — using recommended builder:
let tsfn = callback.build_threadsafe_function()
    .callee_handled::<false>()     // replaces ErrorStrategy::Fatal
    .weak::<true>()                // replaces .unref(env)
    .build()?;                     // for auto-serializable types
    // or .build_callback(|ctx| ...) for manual JS value construction

// v3 — using deprecated compat-mode JsFunction::create_threadsafe_function:
// (only available with compat-mode; signature changed — no queue_size param)
callback.create_threadsafe_function(|ctx: ThreadsafeCallContext<T>| { ... })
```

Files: `context.rs`, `call_override.rs`, `logger.rs`, `config.rs`, `edr_napi_core/subscription.rs`

#### `call_with_return_value` callback signature (4 call sites)

```rust
// v2:
tsfn.call_with_return_value(value, mode, |ret: Vec<String>| { ... });
// v3:
tsfn.call_with_return_value(value, mode, |ret: Result<Vec<String>>, _env: Env| {
    let ret = ret?;
    ...
});
// v3 alternative — async (cleaner, eliminates mpsc::channel pattern):
let ret = tsfn.call_async(value).await?;
```

Files: `logger.rs` (2 sites), `call_override.rs`, `config.rs` (2 sites)

#### `JsUnknown` → `Unknown<'_>` (1 file)

```rust
// v2:
pub type DynJsValueConstructor = dyn FnOnce(&napi::Env) -> napi::Result<JsUnknown>;
// v3:
pub type DynJsValueConstructor = dyn FnOnce(&napi::Env) -> napi::Result<Unknown<'_>>;
```

File: `edr_napi_core/subscription.rs`

#### `JsFunction` → `Function<'_, Args, Ret>` (7 fields/params)

| File | Field | Current | Replacement |
|------|-------|---------|-------------|
| `logger.rs` | `decode_console_log_inputs_callback` | `JsFunction` | `Function<'_, Vec<Buffer>, Vec<String>>` |
| `logger.rs` | `print_line_callback` | `JsFunction` | `Function<'_, (String, bool), ()>` |
| `config.rs` | `on_collected_coverage_callback` | `JsFunction` | `Function<'_, Vec<Buffer>, Promise<()>>` |
| `config.rs` | `on_collected_gas_report_callback` | `JsFunction` | `Function<'_, GasReport, Promise<()>>` |
| `context.rs` | `on_test_suite_completed_callback` | `JsFunction` | `Function<'_, SuiteResult, ()>` |
| `provider.rs` | `call_override_callback` | `JsFunction` | `Function<'_, (Buffer, Buffer), Promise<Option<CallOverrideResult>>>` |
| `subscription.rs` | `subscription_callback` | `JsFunction` | `Function<'_, SubscriptionEvent, ()>` |

#### `JsObject` → `Object<'_>` for deferred promises (3 files)

```rust
// v2:
fn method(&self, env: Env, ...) -> napi::Result<JsObject> {
    let (deferred, promise) = env.create_deferred()?;
    Ok(promise)
}
// v3 (create_deferred still exists, return type changes):
fn method(&self, env: Env, ...) -> napi::Result<Object<'_>> {
    let (deferred, promise) = env.create_deferred()?;
    Ok(promise)
}
```

Files: `context.rs`, `provider.rs`, `mock/time.rs`. Keep `#[napi(ts_return_type = "Promise<T>")]`.

#### `JsString` → `OpaqueString` newtype (1 file)

`config.rs:248` uses `Vec<JsString>` for `owned_accounts` intentionally — `JsString` lacks `Debug`/`Display`/`Serialize`, preventing accidental secret key leakage. Create a newtype:

```rust
pub struct OpaqueString(String);
// Implement FromNapiValue but NOT Debug, Display, or Serialize
```

#### Deprecated `Env` methods (used in ThreadsafeCallContext callbacks)

| Deprecated | Replacement |
|---|---|
| `env.create_object()` | `Object::new(env)` |
| `env.create_arraybuffer_with_data(data)` | `ArrayBuffer::from_data(data)` |
| `env.create_buffer_with_data(data)` | `BufferSlice::from_data(data)` |
| `env.create_bigint_from_words(sign, words)` | `BigInt` constructor |
| `env.create_array_with_length(len)` | `Array` constructor |
| `env.get_boolean(val)` | Use native `bool` |
| `env.create_string_from_std(s)` | Use native `String` |
| `value.into_unknown()` | Use native types directly |

These are used inside `ThreadsafeCallContext` callbacks in `logger.rs`, `config.rs`, `call_override.rs`, and `edr_napi_core/subscription.rs`. They still compile in v3 (just deprecated), so migrating them is optional in any phase.

---

## Migration strategy trade-off

There are two viable approaches. Both require the same total work — the difference is how it's split across PRs.

### Option A: Single PR (recommended)

Do everything in one PR: infrastructure + all Rust code changes + no `compat-mode`.

**Pros:**
- Every call site is touched exactly once — no rework.
- No temporary dependency on `compat-mode` or deprecated APIs.
- The codebase goes from v2-idiomatic directly to v3-idiomatic.
- Simpler to reason about: one PR, one review, one CI validation.

**Cons:**
- Large PR (~10 files with significant Rust changes + infrastructure).
- If something goes wrong, harder to bisect which change caused it.
- Reviewer needs to understand both the infrastructure changes and the ThreadsafeFunction API redesign at once.

**Estimated scope:**
- Infrastructure: 4 files (Cargo.toml x2, package.json, build scripts)
- ThreadsafeFunction + `JsFunction` → `Function`: 6 files
- `JsObject` → `Object`: 3 files
- `JsString` → `OpaqueString`: 1 file
- `JsUnknown` → `Unknown`: 1 file
- Deprecated Env methods: 4 files (within the ThreadsafeFunction callbacks)
- Auto-generated: `index.js`, `index.d.ts`

### Option B: Minimal migration PR + follow-up PRs

Enable `compat-mode` and do the minimum Rust changes required to compile, then clean up in follow-up PRs.

**What `compat-mode` buys:** `JsFunction`, `JsObject`, and `JsBoolean` remain available, so those type replacements can be deferred.

**What `compat-mode` does NOT buy:** `ErrorStrategy`, `ThreadsafeFunction` generics, `create_threadsafe_function` signature, `call_with_return_value` signature, and `JsUnknown` all changed regardless. These must be fixed in the first PR.

This means the first PR still touches every ThreadsafeFunction call site. But with `compat-mode`, it can use the deprecated `JsFunction::create_threadsafe_function(callback)` (new v3 signature, no queue_size) instead of migrating to `Function::build_threadsafe_function()`. This keeps `JsFunction` field types unchanged in the first PR.

**PR 1 — Minimal migration (with `compat-mode`):**
- Infrastructure changes (deps, package.json, CLI flags, scripts)
- Add `compat-mode` feature flag
- Remove `ErrorStrategy` usage, update ThreadsafeFunction type params
- Remove queue_size arg from `create_threadsafe_function` calls
- Update `call_with_return_value` callback signatures
- Replace `JsUnknown` → `Unknown<'_>`
- Verify `.into_unknown()` still works on compat-mode types

**PR 2 — `JsFunction` → `Function` + `build_threadsafe_function()` builder:**
- Replace all `JsFunction` field/param types with `Function<'_, Args, Ret>`
- Migrate `create_threadsafe_function` → `build_threadsafe_function().build()` / `.build_callback()`
- Replace `.unref(env)` → `.weak::<true>()` builder
- Optionally replace `call_with_return_value` → `call_async`

**PR 3 — Remaining compat-mode removals:**
- `JsObject` → `Object<'_>` (deferred promises, 3 files)
- `JsString` → `OpaqueString` (1 file)
- Replace deprecated Env methods
- Remove `compat-mode` feature flag

**Pros:**
- Smaller, more focused PRs — easier to review individually.
- PR 1 validates that v3 infrastructure works across all 7 platforms before touching more code.
- If a platform-specific issue surfaces, it's isolated to the infrastructure PR.

**Cons:**
- PR 1 still requires touching every ThreadsafeFunction call site (the `ErrorStrategy` removal forces this), so it's not as small as a pure infrastructure PR.
- The ThreadsafeFunction call sites are touched twice: once to fix the v3 compat-mode signature (PR 1), then again to migrate to the recommended builder API (PR 2). This is strictly redundant work.
- Temporary reliance on `compat-mode` and deprecated APIs that will be removed in the next PR anyway.
- Three PRs to review and land instead of one.

### Recommendation

**Option A (single PR)** unless the team strongly prefers smaller PRs for review ergonomics. The key insight is that `compat-mode` doesn't buy much here — the ThreadsafeFunction changes (which are the bulk of the Rust work) must happen in the first PR regardless, and migrating to `build_threadsafe_function()` requires changing `JsFunction` → `Function` at the same call sites. Doing it in two passes means editing the same lines twice.

---

## Verification checklist (either approach)

- [ ] `cargo check` passes
- [ ] `pnpm run build:debug` succeeds
- [ ] `diff` old vs new `index.d.ts` shows no public API changes
- [ ] `pnpm test` passes locally
- [ ] All 7 platform builds pass in CI
- [ ] `pnpm publish --dry-run` succeeds

---

## CI Workflow Changes

**File:** `.github/workflows/edr-npm-release.yml`

Build script and prepublish changes propagate automatically. No other CI changes needed — `napi artifacts`, artifact naming (`edr.*.node`), and platform package structure are all unchanged in v3.

### Future: CI Cross-Compilation Opportunity

v3's `napi build --cross-compile` (`-x`) uses `cargo-zigbuild` (Linux) and `cargo-xwin` (Windows). Could eliminate all 4 Docker-based Linux builds:

| Current (Docker) | Potential (cross-compile) |
|---|---|
| Docker on `ubuntu-24.04` for `x86_64-linux-gnu` | `napi build -x` on `ubuntu-24.04` |
| Docker on `ubuntu-24.04-arm` for `aarch64-linux-gnu` | `napi build -x --target aarch64-unknown-linux-gnu` on `ubuntu-24.04` |
| Docker on `ubuntu-24.04` for `x86_64-linux-musl` | `napi build -x --target x86_64-unknown-linux-musl` on `ubuntu-24.04` |
| Docker on `ubuntu-24.04-arm` for `aarch64-linux-musl` | `napi build -x --target aarch64-unknown-linux-musl` on `ubuntu-24.04` |

**Caveats**: Marked as `[experimental]` in v3 CLI. EDR's native C dependencies may complicate cross-compilation. ARM test jobs still need ARM runners. Evaluate as a separate follow-up after migration is stable.

---

## Risk summary

| Risk | Severity | Notes |
|------|----------|-------|
| `ErrorStrategy` removed (no compat shim) | **Compile error** | Must change ThreadsafeFunction type annotations in 6 files |
| `create_threadsafe_function` signature changed | **Compile error** | Must remove queue_size arg at 7 call sites |
| `call_with_return_value` callback signature changed | **Compile error** | Must add `Result` wrapper + `Env` param at 4 call sites |
| `JsUnknown` removed (no compat shim) | **Compile error** | Replace with `Unknown<'_>` in 1 file |
| CLI commands/flags renamed | **Build failure** | `prepublish` → `pre-publish`, remove `--no-const-enum`, `--cargo-flags` → `--` |
| `package.json` config format changed | **Build failure** | `name` → `binaryName`, `triples` → `targets` |
| `into_unknown()` on compat types | **Low** | Verify still works; if not, rewrite callback with direct types |
| TS type generation for `Function<A, R>` | **Low** | Keep `ts_type` overrides as safety net |
| Feature flags changed | **Resolved** | All flags (`napi8`, `async`, `error_anyhow`, `serde-json`) unchanged |
