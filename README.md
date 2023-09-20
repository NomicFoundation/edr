# EDR - Ethereum Development Runtime

**EDR**, or **Ethereum Development Runtime** in full, is a library for creating developer tooling on top of the Ethereum Virtual Machine (EVM), such as an EVM debugger or state inspector.
EDR provides a performant API, written in Rust, with bindings for the Node API (TypeScript).

EDR finds its origins in Hardhat but will be a complete rewrite of our Hardhat Network TypeScript code to Rust, incorporating all of the lessons we have learned over the years, and much more to come.
Currently, it exists within an experimental branch of the [Hardhat monorepo](https://github.com/NomicFoundation/hardhat/tree/rethnet/main/), where we have started to incrementally embed EDR's runtime components into Hardhat.

> **⚠️ Beware**
> 
> This repository is only used for issues at the moment. Please take a look at the [EDR GitHub Project](https://github.com/orgs/NomicFoundation/projects/3) for more details.
> If you are interested in EDR's source code, please have a look at the `rethnet/main` branch of the [Hardhat repository](https://github.com/NomicFoundation/hardhat/tree/rethnet/main/) instead.
