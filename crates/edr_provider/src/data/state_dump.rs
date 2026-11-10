use std::collections::{BTreeMap, BTreeSet};

use edr_blockchain_api::LocalStateDiffs;
use edr_primitives::{Address, Bytecode, KECCAK_EMPTY, U256};
use edr_state_api::{AccountModifierFn, EvmStorageSlot, StateDiff, StateOverride, TransactionId};
use edr_state_dump::{DumpedAccount, StateDump};

use super::ProviderData;
use crate::{
    spec::SyncProviderSpec, time::TimeSinceEpoch, ProviderError, ProviderErrorForChainSpec,
};

impl<ChainSpecT, TimerT> ProviderData<ChainSpecT, TimerT>
where
    ChainSpecT: SyncProviderSpec<TimerT>,
    TimerT: Clone + TimeSinceEpoch,
{
    /// Dumps the accounts of the latest state.
    pub fn dump_state(&mut self) -> Result<StateDump, ProviderErrorForChainSpec<ChainSpecT>> {
        if self.fork_metadata().is_some() {
            return Err(ProviderError::DumpStateUnsupportedInForkMode);
        }

        // The state trie stores hashed storage indices, so the indices come from the
        // diffs that built the state.
        let mut storage_indices: BTreeMap<Address, BTreeSet<U256>> = BTreeMap::new();
        let diffs = LocalStateDiffs::local_state_diffs(&*self.blockchain)
            .iter()
            .map(|(_, diff)| diff)
            .chain(
                self.irregular_state
                    .state_overrides()
                    .values()
                    .map(|state_override| &state_override.diff),
            );
        for diff in diffs {
            for (address, account) in diff.as_inner() {
                storage_indices
                    .entry(*address)
                    .or_default()
                    .extend(account.storage.keys());
            }
        }

        let state = self.current_state()?;
        let mut accounts = BTreeMap::new();
        for (address, indices) in storage_indices {
            let Some(account_info) = state.basic(address)? else {
                continue;
            };

            let mut storage = BTreeMap::new();
            for index in indices {
                let value = state.storage(address, index)?;
                if !value.is_zero() {
                    storage.insert(index, value);
                }
            }

            let code = if account_info.code_hash == KECCAK_EMPTY {
                None
            } else {
                Some(state.code_by_hash(account_info.code_hash)?)
            };

            let account = DumpedAccount {
                balance: account_info.balance,
                code: code.map(|code| code.original_bytes()).unwrap_or_default(),
                nonce: account_info.nonce,
                storage,
            };
            if account != DumpedAccount::default() {
                accounts.insert(address, account);
            }
        }

        Ok(StateDump { accounts })
    }

    /// Merges the accounts of a dump into the latest state.
    ///
    /// The balance and code of each account in the dump replace the current
    /// ones, its nonce is the higher of the two, and its storage slots are set
    /// one by one. Accounts and storage slots that aren't in the dump are kept.
    pub fn load_state(
        &mut self,
        dump: StateDump,
    ) -> Result<(), ProviderErrorForChainSpec<ChainSpecT>> {
        // We clone to automatically revert in case of subsequent errors.
        let mut modified_state = (*self.current_state()?).clone();
        let mut diff = StateDiff::default();

        for (address, account) in dump.accounts {
            let code = (!account.code.is_empty()).then(|| Bytecode::new_raw(account.code));
            let modifier_code = code.clone();
            let mut account_info = modified_state.modify_account(
                address,
                AccountModifierFn::new(Box::new(move |balance, nonce, account_code| {
                    *balance = account.balance;
                    *nonce = (*nonce).max(account.nonce);
                    account_code.clone_from(&modifier_code);
                })),
            )?;

            // The code is stripped from the returned account, but the irregular state
            // needs it.
            account_info.code = code;

            // A storage change on an account that isn't in the diff yet marks it as
            // created, which clears its other storage slots when the state is rebuilt
            // from the diffs. Recording the account change first avoids that, except
            // for an account with only storage: it would be removed as empty, so it's
            // marked as created, like `hardhat_setStorageAt` does.
            if account.storage.is_empty() || !account_info.is_empty() {
                diff.apply_account_change(address, account_info.clone());
            }

            for (index, value) in account.storage {
                let old_value = modified_state.set_account_storage_slot(address, index, value)?;
                let slot = EvmStorageSlot::new_changed(old_value, value, TransactionId::ZERO);
                diff.apply_storage_change(address, index, slot, Some(account_info.clone()));
            }
        }

        let state_root = modified_state.state_root()?;

        self.mem_pool.update(&modified_state)?;

        let block_number = self.blockchain.last_block_number();
        let state_override = self
            .irregular_state
            .state_override_at_block_number(block_number)
            .or_insert_with(|| StateOverride::with_state_root(state_root));
        state_override.diff.apply_diff(diff.into());
        state_override.state_root = state_root;

        self.add_state_to_cache(modified_state, block_number);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use edr_block_header::HeaderOverrides;
    use edr_chain_l1::L1ChainSpec;
    use edr_primitives::{address, Address, U256};
    use edr_state_dump::{DumpedAccount, StateDump};

    use crate::test_utils::ProviderTestFixture;

    const ACCOUNT: Address = address!("0x00000000000000000000000000000000000000aa");

    fn dump(balance: u64) -> StateDump {
        StateDump {
            accounts: BTreeMap::from([(
                ACCOUNT,
                DumpedAccount {
                    balance: U256::from(balance),
                    storage: BTreeMap::from([(U256::from(1), U256::from(7))]),
                    ..DumpedAccount::default()
                },
            )]),
        }
    }

    /// Reads a storage slot from the latest state, rebuilt from the diffs
    /// instead of taken from the state cache.
    fn rebuilt_storage(
        fixture: &ProviderTestFixture<L1ChainSpec>,
        index: u64,
    ) -> anyhow::Result<U256> {
        let data = &fixture.provider_data;
        let state = data.blockchain.state_at_block_number(
            data.last_block_number(),
            data.irregular_state.state_overrides(),
        )?;
        Ok(state.storage(ACCOUNT, U256::from(index))?)
    }

    #[test]
    fn rebuilt_state_merges_the_load() -> anyhow::Result<()> {
        let mut fixture = ProviderTestFixture::<L1ChainSpec>::new_local()?;
        let data = &mut fixture.provider_data;
        data.set_balance(ACCOUNT, U256::from(1))?;
        data.set_account_storage_slot(ACCOUNT, U256::from(2), U256::from(8))?;
        data.mine_and_commit_block(HeaderOverrides::default())?;

        data.load_state(dump(1))?;

        assert_eq!(rebuilt_storage(&fixture, 1)?, U256::from(7));
        assert_eq!(rebuilt_storage(&fixture, 2)?, U256::from(8));

        Ok(())
    }

    #[test]
    fn rebuilt_state_keeps_an_account_with_only_storage() -> anyhow::Result<()> {
        let mut fixture = ProviderTestFixture::<L1ChainSpec>::new_local()?;
        fixture
            .provider_data
            .mine_and_commit_block(HeaderOverrides::default())?;

        fixture.provider_data.load_state(dump(0))?;

        assert_eq!(rebuilt_storage(&fixture, 1)?, U256::from(7));

        Ok(())
    }
}
