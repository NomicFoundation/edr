fn main() -> anyhow::Result<()> {
    let eth_hash = edr_eth2::B256::ZERO;
    let opt_hash = edr_opt::B256::ZERO;

    if eth_hash == opt_hash {
        println!("The same hash");
    }

    let eth_spec_id = edr_eth2::SpecId::ECOTONE;
    let opt_spec_id = edr_opt::SpecId::LATEST;

    if eth_spec_id == opt_spec_id {
        println!("The same spec id");
    }

    Ok(())
}
