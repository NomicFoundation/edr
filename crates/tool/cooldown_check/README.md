# Cargo dependencies cooldown check

Tool for validating that all project dependencies are at least `cooldown_minutes` minutes old.

This tool is inspired by [cargo-cooldown](https://github.com/dertin/cargo-cooldown).

## Motivation

Have an automated way to check and fail when any of the dependencies in the Cargo.lock file are newer than the configured cooldown period.

## What to do if the check fails?

If running `cargo add <version-number>` locks a newer version that violates the cooldown period, you can tell cargo to update (actually downgrade) and lock a specific version by running

```sh
cargo update -p <dependency> --precise <cool_version>
```
