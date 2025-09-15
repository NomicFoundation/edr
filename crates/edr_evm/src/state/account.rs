use edr_state::{account::AccountInfo, AccountStorage};

#[derive(Clone, Debug, Default)]
pub struct EdrAccount {
    pub info: AccountInfo,
    pub storage: AccountStorage,
}

impl From<AccountInfo> for EdrAccount {
    fn from(info: AccountInfo) -> Self {
        Self {
            info,
            storage: AccountStorage::default(),
        }
    }
}
