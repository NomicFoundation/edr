---
"@nomicfoundation/edr": minor
---

Added support for contract-level inline configurations in Solidity tests.

As part of this, the `function` field of `InlineConfigDirectiveError` and `InlineConfigDirectiveLocation` changed from `string` to `string | undefined`: it is absent when the directive is contract-level rather than function-level.
