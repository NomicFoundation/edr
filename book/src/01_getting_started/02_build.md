# Build

EDR exists in a mono-repo. In order to test EDR with Hardhat, we use a pnpm workspace.

To get started, install all dependencies in the root directory:

```bash
pnpm install --frozen-lockfile
```

The Rust build may require installing OpenSSL development and build dependencies.
