---
"@nomicfoundation/edr": minor
---

- Added the `ContractDecoder` type
- Added a function `Provider.contractDecoder` to retrieve the provider's `ContractDecoder` instance
- Changed `EdrContext.createProvider` to receive a `ContractDecoder` instance instead of a `TracingConfigWithBuffers`
  - The `ContractDecoder` can be constructed from a `TracingConfigWithBuffers` using the static method `ContractDecoder.withContracts`
- Changed `Provider.addCompilationResult` to no longer return a `boolean`. Failures are now signaled by throwing an exception.
