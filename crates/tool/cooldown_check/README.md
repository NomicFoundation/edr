# Cargo dependencies cooldown check

Tool for validating that all workspace dependencies are at least `cooldown_minutes` minutes old.

Inspired by [cargo-cooldown](https://github.com/dertin/cargo-cooldown).

## Motivation

Supply-chain attacks often rely on developers adopting a malicious crate version shortly after publication. This tool provides an automated check that flags any dependency in `Cargo.lock` newer than a configurable cooldown period, giving the community time to identify and report compromised releases.

## Goals

- Fail if any dependency violates the cooldown period
- Identify which dependencies fail the check
- Suggest candidate versions that satisfy the cooldown

## Non-goals

- Automatically update `Cargo.lock` with older dependencies
- Create `Cargo.lock` if it is not present

## Usage

### Running the tool

```sh
# Run from the workspace root
cargo cooldown-check

# Verbose output
cargo cooldown-check -- -v
```

### Configuration

Workspace configuration is defined in `<workspace_root>/.cargo/cooldown.toml`:

```toml
cooldown_minutes = 10080  # 7 days
# cache_dir = "/tmp/cooldown-cache"  # optional
# cache_ttl_seconds = 86400           # optional, defaults to 1 day
```

Allowlist rules can lower the effective cooldown per crate or permit an explicit version via `<workspace_root>/.cargo/cooldown-allowlist.toml`:

```toml
# Skip cooldown for a specific crate version
[[allow.exact]]
crate = "foo"
version = "1.2.3"

# Lower the cooldown for a specific crate (minutes)
[[allow.package]]
crate = "tokio"
minutes = 1440
```

### Failure output

The tool only identifies dependencies that violate the cooldown period and suggests actions — it never modifies `Cargo.lock` itself.

When a dependency fails the check, one of two things happens:

- **Candidate versions exist**: the tool lists older versions that satisfy the cooldown and prints a command to downgrade:
  ```
  cargo update <crate> --precise <version>
  ```
- **No candidate versions exist**: no published version is both old enough and compatible with the semver constraints in the dependency graph. The tool suggests to relax the constraints, wait for the cooldown to elapse, or allowlist the crate.

Candidate versions will:

- Not be yanked
- Satisfy all observed semver requirements (across every dependent in the graph)
- Be older than the current lockfile entry
- Have been published before the cooldown cutoff

### Technical details

- The tool invokes `cargo metadata` to read the full dependency graph and records every `VersionReq` that parents impose on their children.
- For each crate sourced from a watched registry, it fetches publication metadata from the crates.io HTTP API through a small on-disk cache and computes the package age.

## Limitations

- Only dependencies sourced from the crates.io registry are checked. Packages from other registries, git sources, or local paths are silently skipped.
- Configuration file paths are not configurable (`cooldown.toml` and `cooldown-allowlist.toml`).
- Configuration is only possible through files; environment variables are not supported.

## References

- [Cargo resolver — SemVer-breaking patch releases](https://doc.rust-lang.org/cargo/reference/resolver.html#semver-breaking-patch-release-breaks-the-build)

## TODO

- Extract into its own repo
  - expose it as a `cargo-` bin crate so it can be executed as a cargo tool
  - CliArgs parsing will have to change to adapt to this (when Cargo invokes `cargo cooldown-check`, it passes `"cooldown-check"` as the first CLI argument.)
