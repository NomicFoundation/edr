---
"@nomicfoundation/edr": minor
---

New `ProviderConfig.baseFeeConfig` field available for configuring different values of eip-1559 `maxChangeDenominator` and `elasticityMultiplier` field.

Configuration example:

```ts
baseFeeConfig: [{
    keyType: BaseFeeActivationType.BlockNumber,
    activation: BigInt(0),
    maxChangeDenominator: BigInt(50),
    elasticityMultiplier: BigInt(6)
},
{
    keyType: BaseFeeActivationType.Hardfork,
    activation: opHardforkToString(OpHardfork.Canyon),
    maxChangeDenominator: BigInt(250),
    elasticityMultiplier: BigInt(6)
},
{
    keyType: BaseFeeActivationType.BlockNumber,
    activation: BigInt(135_513_416),
    maxChangeDenominator: BigInt(250),
    elasticityMultiplier: BigInt(4)
}]
```
