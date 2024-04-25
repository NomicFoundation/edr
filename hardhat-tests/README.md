# EDR Hardhat tests

This package contains a copy of Hardhat core tests (at commit `89a907afb58ec1c371edb094ca357d4489eb1238`), adapted to be run with the code from an installed version of Hardhat. The EDR dependency of this installed version of Hardhat is in turn overridden to use the workspace version in this repo (see `pnpm.overrides` field in the `package.json` at the root of this project).
