# Setup

## Dev Container

To make the developer experience as seamless and consistent as possible, we recommend using the VS Code [devcontainer](https://github.com/NomicFoundation/slang/tree/main/.devcontainer) included in this repository. It is a light image that uses a script to install the minimum required tools to build this project. If you are not familiar with containerized development, we recommend taking a look at [the official VS Code guide](https://code.visualstudio.com/docs/remote/containers). Using a devcontainer allows us to quickly setup the environment and install different dependencies for different projects, without polluting the local environment. In the future, it will enable us to include Windows and Mac OS specific images for cross-platform testing.

We recommend to use the devcontainer setup by using the `Clone repository in named container volume` option. This setup will ensure that the whole environment is isolated from your local host and therefore is platform-consistent, while ensuring persistent storage over container rebuilds or docker restarts.
When using this option we recommend enabling ssh agent forwarding to share your ssh keys with the contaier in a safe way. You can read more about this in the [official VS Code guide](https://code.visualstudio.com/remote/advancedcontainers/sharing-git-credentials#_using-ssh-keys)
Also, beware that VSCode performs a shallow clone of the repository when setting up the volume, so you may want to unshallow it by runnung `git fetch --unshallow`

If you're using rust-analyzer in VS Code and find that the analysis takes too much time, adding the following to your `settings.json` file will help speed things up significantly:

```json
"rust-analyzer.procMacro.ignored": {
    "napi-derive": [
        "napi"
    ],
},
```

Note: To prevent crashes in rust-analyzer, make sure that the container has enough memory. 16GB should be enough for most use cases.

## Automated

If you would like to develop outside a container, you can use the automated setup method provided for your platform.

### Linux

Run the `scripts/setup.sh` script. This script is intended to be reused by the devcontainer and CI.

## Manual

If you would like to set up the environment manually, you will need to install the following dependencies:

- Install Rust using [rustup](https://www.rust-lang.org/tools/install)
- Install Rust fmt nightly version (as long as `unstable_features = true` in `rustfmt.toml`)

  ```sh
   rustup toolchain install nightly --profile minimal --component rustfmt
  ```

- [NodeJS 22](https://nodejs.org/en)
- [pnpm](https://pnpm.io/installation)
