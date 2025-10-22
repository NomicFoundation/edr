# Using pnpm link

When working on something that needs changes in both EDR and Hardhat (or just something that needs EDR to be locally installed), using [`pnpm link`](https://pnpm.io/cli/link) is the recommended approach.

To use the local version of EDR in Hardhat, you need to go to the `hardhat-core` directory and link your local `edr_napi` package:

```bash
cd ~/repos/hardhat/packages/hardhat-core
pnpm install --frozen-lockfile
pnpm link ~/repos/edr/crates/edr_napi
pnpm build
```

You can then use the same process to link Hardhat in some Hardhat-based project[^1]:

```bash
cd ~/repos/some-hardhat-project
pnpm install --frozen-lockfile
pnpm link ~/repos/hardhat/packages/hardhat-core

# this task will now use the local versions of Hardhat and EDR
pnpm hardhat test
```

---

[^1]: This assumes that the Hardhat-based project uses pnpm, which is not always the case. But, if needed, migrating a project from npm/yarn to pnpm is normally straightforward.
