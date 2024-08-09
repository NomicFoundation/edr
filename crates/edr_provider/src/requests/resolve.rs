use edr_eth::{chain_spec::L1ChainSpec, BlockSpec, Bytes, SpecId, U256};
use edr_evm::{state::StateOverrides, transaction};
use edr_rpc_eth::{CallRequest, EstimateGasRequest};

use crate::{data::ProviderData, spec::ResolveRpcType, time::TimeSinceEpoch, ProviderError};

use super::validation::validate_call_request;

impl<TimerT: Clone + TimeSinceEpoch> ResolveRpcType<L1ChainSpec, TimerT, transaction::Request>
    for CallRequest
{
    fn resolve_rpc_type(
        self,
        data: &mut ProviderData<L1ChainSpec, TimerT>,
        block_spec: &BlockSpec,
        state_overrides: &StateOverrides,
    ) -> Result<transaction::Request, ProviderError<L1ChainSpec>> {
        resolve_call_request(
            data,
            self,
            block_spec,
            state_overrides,
            |_data| Ok(U256::ZERO),
            |_, max_fee_per_gas, max_priority_fee_per_gas| {
                let max_fee_per_gas = max_fee_per_gas
                    .or(max_priority_fee_per_gas)
                    .unwrap_or(U256::ZERO);

                let max_priority_fee_per_gas = max_priority_fee_per_gas.unwrap_or(U256::ZERO);

                Ok((max_fee_per_gas, max_priority_fee_per_gas))
            },
        )
    }
}

impl<TimerT: Clone + TimeSinceEpoch> ResolveRpcType<L1ChainSpec, TimerT, transaction::Request>
    for EstimateGasRequest
{
    fn resolve_rpc_type(
        self,
        data: &mut ProviderData<L1ChainSpec, TimerT>,
        block_spec: &BlockSpec,
        state_overrides: &StateOverrides,
    ) -> Result<transaction::Request, ProviderError<L1ChainSpec>> {
        resolve_call_request(
            data,
            self.inner,
            block_spec,
            state_overrides,
            ProviderData::gas_price,
            |data, max_fee_per_gas, max_priority_fee_per_gas| {
                let max_priority_fee_per_gas = max_priority_fee_per_gas.unwrap_or_else(|| {
                    const DEFAULT: u64 = 1_000_000_000;
                    let default = U256::from(DEFAULT);

                    if let Some(max_fee_per_gas) = max_fee_per_gas {
                        default.min(max_fee_per_gas)
                    } else {
                        default
                    }
                });

                let max_fee_per_gas = max_fee_per_gas.map_or_else(
                    || -> Result<U256, ProviderError<L1ChainSpec>> {
                        let base_fee = if let Some(block) = data.block_by_block_spec(block_spec)? {
                            max_priority_fee_per_gas
                                + block.header().base_fee_per_gas.unwrap_or(U256::ZERO)
                        } else {
                            // Pending block
                            let base_fee = data.next_block_base_fee_per_gas()?.expect(
                                "This function can only be called for post-EIP-1559 blocks",
                            );

                            U256::from(2) * base_fee + max_priority_fee_per_gas
                        };

                        Ok(base_fee)
                    },
                    Ok,
                )?;

                Ok((max_fee_per_gas, max_priority_fee_per_gas))
            },
        )
    }
}

fn resolve_call_request<TimerT: Clone + TimeSinceEpoch>(
    data: &mut ProviderData<L1ChainSpec, TimerT>,
    request: CallRequest,
    block_spec: &BlockSpec,
    state_overrides: &StateOverrides,
    default_gas_price_fn: impl FnOnce(
        &ProviderData<L1ChainSpec, TimerT>,
    ) -> Result<U256, ProviderError<L1ChainSpec>>,
    max_fees_fn: impl FnOnce(
        &ProviderData<L1ChainSpec, TimerT>,
        // max_fee_per_gas
        Option<U256>,
        // max_priority_fee_per_gas
        Option<U256>,
    ) -> Result<(U256, U256), ProviderError<L1ChainSpec>>,
) -> Result<transaction::Request, ProviderError<L1ChainSpec>> {
    validate_call_request(data.evm_spec_id(), &request, &block_spec)?;

    let CallRequest {
        from,
        to,
        gas,
        gas_price,
        max_fee_per_gas,
        max_priority_fee_per_gas,
        value,
        data: input,
        access_list,
        ..
    } = request;

    let chain_id = data.chain_id();
    let sender = from.unwrap_or_else(|| data.default_caller());
    let gas_limit = gas.unwrap_or_else(|| data.block_gas_limit());
    let input = input.map_or(Bytes::new(), Bytes::from);
    let nonce = data.nonce(&sender, Some(block_spec), state_overrides)?;
    let value = value.unwrap_or(U256::ZERO);

    let evm_spec_id = data.evm_spec_id();
    let request = if evm_spec_id < SpecId::LONDON || gas_price.is_some() {
        let gas_price = gas_price.map_or_else(|| default_gas_price_fn(data), Ok)?;
        match access_list {
            Some(access_list) if evm_spec_id >= SpecId::BERLIN => {
                transaction::Request::Eip2930(transaction::request::Eip2930 {
                    nonce,
                    gas_price,
                    gas_limit,
                    value,
                    input,
                    kind: to.into(),
                    chain_id,
                    access_list,
                })
            }
            _ => transaction::Request::Eip155(transaction::request::Eip155 {
                nonce,
                gas_price,
                gas_limit,
                kind: to.into(),
                value,
                input,
                chain_id,
            }),
        }
    } else {
        let (max_fee_per_gas, max_priority_fee_per_gas) =
            max_fees_fn(data, max_fee_per_gas, max_priority_fee_per_gas)?;
        transaction::Request::Eip1559(transaction::request::Eip1559 {
            chain_id,
            nonce,
            max_fee_per_gas,
            max_priority_fee_per_gas,
            gas_limit,
            kind: to.into(),
            value,
            input,
            access_list: access_list.unwrap_or_default(),
        })
    };

    Ok(request)
}

#[cfg(test)]
mod tests {
    use edr_rpc_eth::CallRequest;

    use super::*;
    use crate::{data::test_utils::ProviderTestFixture, test_utils::pending_base_fee};

    #[test]
    fn resolve_call_request_with_gas_price() -> anyhow::Result<()> {
        let mut fixture = ProviderTestFixture::new_local()?;

        let pending_base_fee = pending_base_fee(&mut fixture.provider_data)?;

        let request = CallRequest {
            from: Some(fixture.nth_local_account(0)?),
            to: Some(fixture.nth_local_account(1)?),
            gas_price: Some(pending_base_fee),
            ..CallRequest::default()
        };

        let resolved = resolve_call_request(
            &mut fixture.provider_data,
            request,
            &BlockSpec::pending(),
            &StateOverrides::default(),
            |_data| unreachable!("gas_price is set"),
            |_, _, _| unreachable!("gas_price is set"),
        )?;

        assert_eq!(*resolved.gas_price(), pending_base_fee);

        Ok(())
    }

    #[test]
    fn resolve_call_request_inner_with_max_fee_and_max_priority_fee() -> anyhow::Result<()> {
        let mut fixture = ProviderTestFixture::new_local()?;

        let max_fee_per_gas = pending_base_fee(&mut fixture.provider_data)?;
        let max_priority_fee_per_gas = Some(max_fee_per_gas / U256::from(2));

        let request = CallRequest {
            from: Some(fixture.nth_local_account(0)?),
            to: Some(fixture.nth_local_account(1)?),
            max_fee_per_gas: Some(max_fee_per_gas),
            max_priority_fee_per_gas,
            ..CallRequest::default()
        };

        let resolved = resolve_call_request(
            &mut fixture.provider_data,
            request,
            &BlockSpec::pending(),
            &StateOverrides::default(),
            |_data| unreachable!("max fees are set"),
            |_, max_fee_per_gas, max_priority_fee_per_gas| {
                Ok((
                    max_fee_per_gas.expect("max fee is set"),
                    max_priority_fee_per_gas.expect("max priority fee is set"),
                ))
            },
        )?;

        assert_eq!(*resolved.gas_price(), max_fee_per_gas);
        assert_eq!(
            resolved.max_priority_fee_per_gas().cloned(),
            max_priority_fee_per_gas
        );

        Ok(())
    }
}
