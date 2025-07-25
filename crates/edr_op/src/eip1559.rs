use edr_eth::{eips::eip1559::ConstantBaseFeeParams, Bytes};

/// Currently used version of the dynamic base fee parameter.
pub const DYNAMIC_BASE_FEE_PARAM_VERSION: u8 = 0x0;

/// Encodes the dynamic base fee parameters into a byte array.
///
/// <https://specs.optimism.io/protocol/holocene/exec-engine.html#dynamic-eip-1559-parameters>
pub fn encode_dynamic_base_fee_params(base_fee_params: &ConstantBaseFeeParams) -> Bytes {
    let denominator: [u8; 4] = u32::try_from(base_fee_params.max_change_denominator)
        .unwrap_or_else(|_| {
            panic!(
                "Base fee denominators can only be up to u32::MAX, but got {}",
                base_fee_params.max_change_denominator
            )
        })
        .to_be_bytes();
    let elasticity: [u8; 4] = u32::try_from(base_fee_params.elasticity_multiplier)
        .unwrap_or_else(|_| {
            panic!(
                "Base fee elasticity can only be up to u32::MAX, but got {}",
                base_fee_params.elasticity_multiplier
            )
        })
        .to_be_bytes();

    let mut extra_data = [0u8; 9];
    extra_data[0] = DYNAMIC_BASE_FEE_PARAM_VERSION;
    extra_data[1..=4].copy_from_slice(&denominator);
    extra_data[5..=8].copy_from_slice(&elasticity);

    let bytes: Box<[u8]> = Box::new(extra_data);
    Bytes::from(bytes)
}
