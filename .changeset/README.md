# Changesets

A changeset records what a PR changes for users of `@nomicfoundation/edr`. On each push to `main`, the release workflow opens a release PR that turns the pending changesets into entries of `crates/edr_napi/CHANGELOG.md`. Write a changeset for those users, not for reviewers.

## When to add one

Every PR adds one changeset, unless it changes nothing published. A PR that only touches tests, CI, benchmarks or repository docs gets the `no changeset needed` label instead. A PR that changes doc comments on the published API needs a changeset. CI fails a PR that has neither a changeset nor the label.

## How to add one

Run `pnpm exec changeset` from the repository root, or write `.changeset/<name>.md` by hand. Name the file after the change, e.g. `fix-stack-trace-console-address.md`. The frontmatter always names only `@nomicfoundation/edr`; the platform packages follow its version.

```markdown
---
"@nomicfoundation/edr": patch
---

Fixed Solidity stack-trace construction to recognize `console.log` calls by the Hardhat console address.
```

## Bump level

- `patch`: a fix, or an addition that leaves existing APIs and documented behavior unchanged.
- `minor`: a change to existing public API or documented behavior, including every breaking change.

EDR is pre-1.0, so breaking changes ship as `minor`.

## Writing the entry

- Open with the past-tense verb that names the kind of change: `Added`, `Changed`, `Deprecated`, `Fixed`, `Removed`, `Improved` or `Upgraded`.
- Name the affected API, option or method, in backticks like any other code, and state the user-visible effect.
- Put a breaking change in its own sentence or bullet that starts with `BREAKING CHANGE:`. Name the replacement.
- When a PR makes several independent changes, list them as bullets in the one changeset. Each bullet gets its own verb.
- Describe behavior, not the implementation. Do not reference PRs or commits; the release tooling adds the commit hash.
- Keep one paragraph per line without hard wraps.

Examples, quoted from the changelog:

```markdown
Added a synchronous, native Keccak-256 implementation: `keccak256`.
```

```markdown
Changed the platform-specific `@nomicfoundation/edr-*` packages from `dependencies` to `optionalDependencies`, so installs only download the build for the current platform instead of all of them. Note: npm < 11.3.0 may skip the platform package when reusing a lockfile created on a different platform (npm/cli#4828); if affected, upgrade with `npm install -g npm@11`.
```

```markdown
Added profile support to Solidity test inline configuration. `SolidityTestRunnerConfigArgs` now accepts `testProfile` and `declaredTestProfiles`. Unprefixed directives apply to every profile, while profile-prefixed directives apply only when that profile is selected and override unprefixed directives with the same key. Undeclared profile prefixes are rejected.

BREAKING CHANGE: Renamed the `InlineConfigUnsupportedProfile` inline config problem to `InlineConfigUndeclaredProfile`. The replacement also includes the declared profile names in `declaredProfiles`.
```
