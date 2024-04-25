use edr_eth2::TestEth;
use edr_opt::TestOpt;

fn main() -> anyhow::Result<()> {
    let mut test_opt = TestOpt::default();
    let mut test_eth = TestEth::default();

    test_eth.evm.transact()?;
    test_opt.evm.transact()?;

    Ok(())
}
