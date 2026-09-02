# Workflow scripts

Scripts run by GitHub Actions, either with `node <file>.ts` or `require()`d from an `actions/github-script` step. Two Bash helpers, `cargo-doc.sh` and `cargo-hack-edr.sh`, are invoked directly by `edr-ci.yml`.

`pnpm test:workflows` and `pnpm tsc:workflows` cover this directory. In CI they are the `Test workflow scripts` step and part of `Run lint script`, both in `edr-ci.yml`'s `Build and lint` job. `test:workflows` deliberately runs _before_ `pnpm install`, which is what keeps the first rule below honest rather than aspirational.

Everything in [`scripts/README.md`](../../scripts/README.md) applies, plus two constraints. Both come from how the workflows load these files: the regression benchmark and the compat-pin validation each check this directory out on its own, never run `pnpm install`, and then `require()` a module from a CommonJS context.

**No dependencies.** Only `node:` builtins are available. There is no `node_modules` in those jobs.

**No top-level `await`.** `require()` of an ESM graph containing one fails with `ERR_REQUIRE_ASYNC_MODULE`.

## Loading from `github-script`

Export a named function; the workflow destructures it:

```js
const { resolveRegressionTrigger } = require(
  `${process.env.GITHUB_WORKSPACE}/.github/scripts/resolve-regression-trigger.ts`
);
await resolveRegressionTrigger({ github, context, core });
```

Nothing validates those strings: Prettier and `tsc` do not parse embedded JS, and actionlint does not read `script:` bodies. A wrong module path or export name fails only when the workflow runs, so check both by hand when you rename either.
