# GHA caches

- repo can store up to 10 GB of caches. Once fulled, older caches are evicted. <https://github.com/actions/cache/blob/main/README.md#cache-limits>
- caches version is determined by path and the compression tool use. So two caches with the same key but different paths or compression tools are two separate caches. <https://github.com/actions/cache/blob/main/README.md#cache-version>
- cache is scoped to the key, version, and branch. The default branch cache is available to other branches. <https://github.com/actions/cache/blob/main/README.md#cache-scopes>
  - PR workflow can access the PR branch caché, the PR base branch caché & the repo default branch caché (`main`)

## Questions

- Update caché by downloading new files to path?
- Perhaps we could save caché only in `main` branch workflows and in the rest just restore it?

- why don't we have a single ETH JSON RPC caché?
- we could use cross OS caché for JSON rpc caché - we store `.json` files <https://github.com/actions/cache/blob/main/tips-and-workarounds.md#cross-os-cache>
- I think that the step "Rust cache" in `EDR Benchmarks` workflow file is redundant since `setup-node` action should already caché al rust dependencies
