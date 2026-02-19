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

- Create `Cargo.lock` if it is not present
- Automatically update `Cargo.lock` with older dependencies

## Usage

This tool validates a workspace dependency graph against a configurable cooldown period — it does not perform any automatic actions on behalf of the workspace maintainer.

### Configuration

- Workspace configuration is defined in `<workspace_root>/.cargo/cooldown.toml`.
- Allowlist rules can lower the effective cooldown per crate or permit an explicit version, via `allow.package` or `allow.exact` sections in `<workspace_root>/.cargo/cooldown-allowlist.toml`.

### Suggested candidates

For each dependency that fails the check, the tool suggests replacement versions. Candidates will:

- Not be yanked
- Satisfy all observed semver requirements (across every dependent in the graph)
- Be older than the current lockfile entry
- Have been published before the cooldown cutoff

### Technical details

- The tool invokes `cargo metadata` to read the full dependency graph and records every `VersionReq` that parents impose on their children.
- For each crate sourced from a watched registry, it fetches publication metadata from the crates.io HTTP API through a small on-disk cache and computes the package age.

## Limitations

- Configuration file paths are not configurable (`cooldown.toml` and `cooldown-allowlist.toml`).
- Configuration is only possible through files; environment variables are not supported.

## References

- [Cargo resolver — SemVer-breaking patch releases](https://doc.rust-lang.org/cargo/reference/resolver.html#semver-breaking-patch-release-breaks-the-build)

## TODO

- Load cache dir from config
- Extract into it's own repo
  - expose it as a `cargo-` bin crate so it can be executed as a cargo tool
  - CliArgs parsing will have to change to adapt to this (when Cargo invokes `cargo cooldown-check`, it passes `"cooldown-check"` as the first CLI argument.)
