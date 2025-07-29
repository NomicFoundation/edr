use alloy_primitives::Address;
use revm::{
    primitives::hash_map::HashMap,
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
            let account = Account {
                info: predeploy.account_info,
                storage: predeploy.storage,
                // Need touched and created to be committed.
                status: AccountStatus::Created | AccountStatus::Touched,
                transaction_id: 0,
            };
            (predeploy.address, account)
        })
        .collect::<HashMap<_, _>>();

    db.commit(changes);
}
