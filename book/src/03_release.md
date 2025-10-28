# Release

Releasing the [EDR NPM package](../../crates/edr_napi/package.json) is handled by the [EDR NPM release](../../.github/workflows/edr-npm-release.yml) GitHub Action workflow.

A new release is created automatically on commits to the `main` branch that follow the following format: `edr-0.1.0` for releases or `edr-0.1.0-alpha.1` for pre-releases.
