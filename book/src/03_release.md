# Release

Releasing the [EDR NPM package](../../crates/edr_napi/package.json) is handled by the [EDR NPM release](../../.github/workflows/edr-npm-release.yml) GitHub Action workflow.

A new release is created automatically on commits to the `main` branch that follow the following format: `edr-0.1.0` for releases or `edr-0.1.0-alpha.1` for pre-releases.

## Workflow structure

The npm pipeline is split across three workflows:

- [`edr-npm-build.yml`](../../.github/workflows/edr-npm-build.yml) — reusable (`workflow_call`) build+test+bundle pipeline: the 7-target build matrix, binding tests, bundle preparation, and the pre-publish review. Its single `release` boolean input is the only release/non-release mode switch: release runs build cold (no cargo cache, asserted at runtime) and pull official Docker Hub images instead of the [GHCR mirror](./02_development/11_ci_docker_mirror.md).
- [`edr-npm-ci.yml`](../../.github/workflows/edr-npm-ci.yml) — PR/branch validation: calls the build workflow with `release: false`. Skips itself on release pushes.
- [`edr-npm-release.yml`](../../.github/workflows/edr-npm-release.yml) — detects release commits ([`check-release-commit`](../../.github/actions/check-release-commit/action.yml)), then calls the build workflow with `release: true` and runs the release-only jobs: cargo cooldown check, Slack notifications, and the `edr-release` environment-protected publish. Everything hangs off a single `release_gate` job, so no individual job or step needs its own release condition.

## Release rehearsal

Dispatching `edr-npm-release.yml` manually (`workflow_dispatch`) runs the full release path on any commit — cold build, Docker Hub images, cooldown check, environment approval — but publishes with `--dry-run`, so nothing reaches the npm registry. Use this to validate release-path changes without cutting a release. Real publishes happen only on push events.
