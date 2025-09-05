
    use std::{str::FromStr, sync::LazyLock};
    
    use edr_evm::hardfork::{self, Activations, ChainConfig, ForkCondition};
    use op_revm::OpSpecId;
    