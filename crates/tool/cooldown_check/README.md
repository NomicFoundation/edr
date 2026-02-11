# Cargo dependencies cooldown check

Tool for validating that all project dependencies are at least `cooldown_minutes` minutes old.

This tool is inspired by [cargo-cooldown](https://github.com/dertin/cargo-cooldown).

## Motivation

Have an automated way to check and fail when any of the dependencies in the Cargo.lock file are newer than the configured cooldown period.

## Goal

- have the command fail if any of the dependecies violates the cooldown period
- identify which are the dependencies that violates the contraint
- suggest dependency version that could work

## Non-Goals

- automatically update `Cargo.lock` with cooler dependencies

## Limitations

- File config files won't be configurable (both `cooldown.toml` and `allowlist.toml`)

## What to do if the check fails?

If running `cargo add <version-number>` locks a newer version that violates the cooldown period, you can tell cargo to update (actually downgrade) and lock a specific version by running

```sh
cargo update -p <dependency> --precise <cool_version>
```

For other alternatives, take a look at <https://doc.rust-lang.org/cargo/reference/resolver.html#semver-breaking-patch-release-breaks-the-build>
