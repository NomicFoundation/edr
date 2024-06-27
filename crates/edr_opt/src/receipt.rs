use edr_eth::B256;

pub enum Data {
    PreEip658Legacy { state_root: B256 },
    PostEip658Legacy { status: u8 },
    Eip2930 { status: u8 },
    Eip1559 { status: u8 },
    Eip4844 { status: u8 },
    Deposited { status: u8, deposit_nonce: u64 },
}
