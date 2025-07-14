use edr_chain_l1::{L1ChainSpec, L1Hardfork};
use edr_eth::{
    account::{Account, AccountInfo, AccountStatus},
    Address, HashMap, B256, U256,
};
use edr_evm::{
    blockchain::{Blockchain as _, BlockchainMut as _, GenesisBlockOptions, LocalBlockchain},
    state::IrregularState,
};

#[test]
fn compute_state_after_reserve() -> anyhow::Result<()> {
    let address1 = Address::random();
    let accounts = vec![(
        address1,
        AccountInfo {
            balance: U256::from(1_000_000_000u64),
            ..AccountInfo::default()
        },
    )];

    let genesis_diff = accounts
        .iter()
        .map(|(address, info)| {
            (
                *address,
                Account {
                    info: info.clone(),
                    storage: HashMap::new(),
                    status: AccountStatus::Created | AccountStatus::Touched,
                },
            )
        })
        .collect::<HashMap<_, _>>()
        .into();

    let mut blockchain = LocalBlockchain::<L1ChainSpec>::new(
        genesis_diff,
        123,
        L1Hardfork::SHANGHAI,
        GenesisBlockOptions {
            gas_limit: Some(6_000_000),
            mix_hash: Some(B256::random()),
            ..GenesisBlockOptions::default()
        },
    )
    .unwrap();

    let irregular_state = IrregularState::default();
    let expected = blockchain.state_at_block_number(0, irregular_state.state_overrides())?;

    blockchain.reserve_blocks(1_000_000_000, 1)?;

    let actual =
        blockchain.state_at_block_number(1_000_000_000, irregular_state.state_overrides())?;

    assert_eq!(actual.state_root().unwrap(), expected.state_root().unwrap());

    for (address, expected) in accounts {
        let actual_account = actual.basic(address)?.expect("account should exist");
        assert_eq!(actual_account, expected);
    }

    Ok(())
}
