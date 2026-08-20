use alloy_primitives::Address;
use revm::{
    state::{Account, AccountInfo, AccountStatus, EvmStorage},
    DatabaseCommit,
};
use serde::{Deserialize, Serialize};

/// A contract that is part of the genesis state of a network.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Predeploy {
    /// The predeploy contract address.
    pub address: Address,
    /// The EVM account info
    pub account_info: AccountInfo,
    /// The EVM storage
    pub storage: EvmStorage,
}

pub(super) fn insert_predeploys(
    mut db: impl DatabaseCommit,
    predeploys: impl IntoIterator<Item = Predeploy>,
) {
    let changes = predeploys
        .into_iter()
        .map(|predeploy| {
            let mut account = Account::from(predeploy.account_info);
            account.storage = predeploy.storage;
            // Need touched and created to be committed.
            account.status = AccountStatus::Created | AccountStatus::Touched;
            (predeploy.address, account)
        })
        .collect();

    db.commit(changes);
}
