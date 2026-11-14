---
"@nomicfoundation/edr": minor
---

Added support for the EIP-7702 delegation cheatcodes in Solidity tests: `vm.signDelegation`, `vm.attachDelegation`, and `vm.signAndAttachDelegation`, with all their overloads. They sign an authorization with the given private key and write the delegation designator to the authority's account, so calls to the authority run the implementation's code. Nothing is broadcast, matching Foundry's behavior in a test context.
