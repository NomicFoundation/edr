# GHA caches

## Context & documentation

- repo can store up to 10 GB of caches. Once fulled, older caches are evicted. <https://github.com/actions/cache/blob/main/README.md#cache-limits>
- caches version is determined by path and the compression tool use. So two caches with the same key but different paths or compression tools are two separate caches. <https://github.com/actions/cache/blob/main/README.md#cache-version>
- cache is scoped to the key, version, and branch. The default branch cache is available to other branches. <https://github.com/actions/cache/blob/main/README.md#cache-scopes>
  - PR workflow can access the PR branch caché, the PR base branch caché & the repo default branch caché (`main`)
- caches are immutable. If a cache is restored, then it cannot be updated. There is a workaround for achieving this by creating a new cache which key has a hash of the cached files
  - <https://github.com/actions/cache/blob/main/tips-and-workarounds.md#update-a-cache>
- cache key matching is done first by `cache-key`, then by `restore-keys`.  The cache action first searches for cache hits for key and the cache version in the branch containing the workflow run. If there is no hit, it searches for prefix-matches for key, and if there is still no hit, it searches for restore-keys and the version
  - <https://docs.github.com/en/actions/reference/workflows-and-actions/dependency-caching#cache-key-matching>

## EDR current scenario

EDR uses cache for:

- Node dependencies: via setup-node action
  - one cache for each platform (~70Mb each)
- Rust dependencies: via setup-rust action (setup-rust-toolchain -> rust-cache)
  - one for each job & platform combination (from ~600MB to 1,4GB each - I guess the difference is size is eplained by the different components that are installed in each job
- EDR-cache for RPC responses
  - specific keys for different jobs & paths. We are defining different keys even when the paths of the `edr-cache` are different, which would already result in a different cache since the path is one of the defining fields for the cache version.
    - `crates/edr_solidity_tests/tests/testdata/edr-cache/solidity-tests/rpc/`
    - `edr-cache/rpc_cache/`
- currently lot of jobs don't find a matching cache. Probably due to cache eviction policy

## Doubts & possible improvements

- why don't we have a single ETH JSON RPC caché?
  - we could use cross OS caché for JSON rpc caché - we store `.json` files that are not specific to the platform <https://github.com/actions/cache/blob/main/tips-and-workarounds.md#cross-os-cache>
  - we could save RPC caché only in `main` branch workflows and in the rest just restore it
  - Or we could force the deletion of closed-PR caches to prevent caches from default branch getting thrashed. <https://github.com/actions/cache/blob/main/tips-and-workarounds.md#force-deletion-of-caches-overriding-default-cache-eviction-policy>
- I think that the step "Rust cache" in `EDR Benchmarks` workflow file is redundant since `setup-rust` action should already caches all rust dependencies
  - <https://github.com/Swatinem/rust-cache?tab=readme-ov-file#cache-details>
- We could define a specific cache key `shared-key` for `setup-rust` job, since it shouldn't change between different workflow files
  - By not defining a specific key, by default we are creating a separete caché for every job
