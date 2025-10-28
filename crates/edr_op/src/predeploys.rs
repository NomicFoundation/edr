use edr_primitives::{address, hex, Address};

/// The address of the `GasPriceOracle` predeploy.
pub const GAS_PRICE_ORACLE_ADDRESS: Address =
    address!("0x420000000000000000000000000000000000000f");

/// The address of the `L2ToL1MessagePasser` predeploy.
pub const L2_TO_L1_MESSAGE_PASSER_ADDRESS: Address =
    address!("0x4200000000000000000000000000000000000016");

/// Returns the bytecode for the `GasPriceOracle` predeploy, introduced during
/// the Ecotone hardfork.
pub fn gas_price_oracle_code_ecotone() -> Vec<u8> {
    hex::decode(include_str!(
        "../data/predeploys/gas_price_oracle/ecotone.txt"
    ))
    .expect("The bytecode for the GasPriceOracle predeploy in the Ecotone hardfork should be a valid hex string")
}

/// Returns the bytecode for the `GasPriceOracle` predeploy, introduced during
/// the Fjord hardfork.
pub fn gas_price_oracle_code_fjord() -> Vec<u8> {
    hex::decode(include_str!(
        "../data/predeploys/gas_price_oracle/fjord.txt"
    ))
    .expect("The bytecode for the GasPriceOracle predeploy in the Fjord hardfork should be a valid hex string")
}

/// Returns the bytecode for the `GasPriceOracle` predeploy, introduced during
/// the Isthmus hardfork.
pub fn gas_price_oracle_code_isthmus() -> Vec<u8> {
    hex::decode(include_str!(
        "../data/predeploys/gas_price_oracle/isthmus.txt"
    ))
    .expect("The bytecode for the GasPriceOracle predeploy in the Isthmus hardfork should be a valid hex string")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gas_price_oracle_code() {
        // Ensure bytecode can be constructed without panics.
        let _ecotone_override = gas_price_oracle_code_ecotone();
        let _fjord_override = gas_price_oracle_code_fjord();
        let _isthmus_override = gas_price_oracle_code_isthmus();
    }
}
