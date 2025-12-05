use edr_eip1559::ConstantBaseFeeParams;
use edr_primitives::Bytes;

/// Post-Holocene version of the dynamic base fee parameter.
pub const HOLOCENE_BASE_FEE_PARAM_VERSION: u8 = 0x0;
/// Post-Jovian version of the dynamic base fee parameter.
pub const JOVIAN_BASE_FEE_PARAM_VERSION: u8 = 0x1;

/// Encodes the dynamic base fee parameters into a byte array following Holocene
/// spec
///
/// <https://specs.optimism.io/protocol/holocene/exec-engine.html#dynamic-eip-1559-parameters>
pub fn encode_dynamic_base_fee_params_holocene(base_fee_params: &ConstantBaseFeeParams) -> Bytes {
    let mut extra_data = [0u8; 9];
    extra_data[0] = HOLOCENE_BASE_FEE_PARAM_VERSION;
    copy_base_fee_params(&mut extra_data, base_fee_params);
    let bytes: Box<[u8]> = Box::new(extra_data);
    Bytes::from(bytes)
}

/// Encodes the dynamic base fee parameters into a byte array following Jovian
/// spec
///
/// <https://specs.optimism.io/protocol/jovian/exec-engine.html#minimum-base-fee-in-block-header>
pub fn encode_dynamic_base_fee_params_jovian(
    base_fee_params: &ConstantBaseFeeParams,
    min_base_fee: u128,
) -> Bytes {
    let min_base_fee_bytes: [u8; 8] = u64::try_from(min_base_fee)
        .unwrap_or_else(|_| {
            panic!("Min base fee can only be up to u64::MAX, but got {min_base_fee}",)
        })
        .to_be_bytes();

    let mut extra_data = [0u8; 17];
    extra_data[0] = JOVIAN_BASE_FEE_PARAM_VERSION;
    copy_base_fee_params(&mut extra_data, base_fee_params);
    extra_data[9..=16].copy_from_slice(&min_base_fee_bytes);
    let bytes: Box<[u8]> = Box::new(extra_data);
    Bytes::from(bytes)
}

/// Copies `base_fee_params` into the supplied byte slice.
/// Panics if `bytes` is shorter than 9 bytes.
#[allow(clippy::indexing_slicing)]
fn copy_base_fee_params(bytes: &mut [u8], base_fee_params: &ConstantBaseFeeParams) {
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
    bytes[1..=4].copy_from_slice(&denominator);
    bytes[5..=8].copy_from_slice(&elasticity);
}
