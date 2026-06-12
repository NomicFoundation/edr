---
"@nomicfoundation/edr": patch
---

Added the optional `gasEstimationMode` provider configuration that controls the success criteria of `eth_estimateGas`: the default `TopLevelSuccess` preserves the previous behavior, while `NoInternalOutOfGas` returns estimations that also avoid internal calls running out of gas, erroring when no such estimation exists.
