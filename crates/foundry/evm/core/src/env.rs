use revm::{
    Context, Database, Journal, JournalEntry,
    context::{CfgEnv, JournalInner, JournalTr},
};

/// Helper container type for `block`, `cfg`, and `tx`.
#[derive(Clone, Debug, Default)]
pub struct Env<BlockT, TxT, HardforkT> {
    pub block: BlockT,
    pub cfg: CfgEnv<HardforkT>,
    pub tx: TxT,
}

/// Helper container type for `block`, `cfg`, and `tx`.
impl<BlockT, TxT, HardforkT> Env<BlockT, TxT, HardforkT>
where
    BlockT: Default,
    HardforkT: Default,
    TxT: Default,
{
    pub fn default_with_spec_id(spec_id: HardforkT) -> Self {
        let mut cfg = CfgEnv::default();
        cfg.spec = spec_id;

        Self::new(cfg, BlockT::default(), TxT::default())
    }

    pub fn new(cfg: CfgEnv<HardforkT>, block: BlockT, tx: TxT) -> Self {
        Self { block, cfg, tx }
    }

    pub fn new_with_spec_id(cfg: CfgEnv<HardforkT>, block: BlockT, tx: TxT, spec_id: HardforkT) -> Self {
        let mut cfg = cfg;
        cfg.spec = spec_id;

        Self::new(cfg, block, tx)
    }
}

/// Helper struct with mutable references to the block and cfg environments.
pub struct EnvMut<'a, BlockT, TxT, HardforkT> {
    pub block: &'a mut BlockT,
    pub cfg: &'a mut CfgEnv<HardforkT>,
    pub tx: &'a mut TxT,
}

impl<'a, BlockT, TxT, HardforkT> EnvMut<'a, BlockT, TxT, HardforkT> {
    /// Returns a copy of the environment.
    pub fn to_owned(&self) -> Env<BlockT, TxT, HardforkT>
    where
        BlockT: Clone,
        HardforkT: Clone,
        TxT: Clone,
    {
        Env {
            block: self.block.to_owned(),
            cfg: self.cfg.to_owned(),
            tx: self.tx.to_owned(),
        }
    }
}

pub trait AsEnvMut {
    type BlockT;
    type HardforkT;
    type TxT;

    fn as_env_mut(&mut self) -> EnvMut<'_, Self::BlockT, Self::TxT, Self::HardforkT>;
}

impl<'a, BlockT, TxT, HardforkT> AsEnvMut for EnvMut<'a, BlockT, TxT, HardforkT> {
    type BlockT = BlockT;
    type HardforkT = HardforkT;
    type TxT = TxT;

    fn as_env_mut(&mut self) -> EnvMut<'_, BlockT, TxT, HardforkT> {
        EnvMut { block: self.block, cfg: self.cfg, tx: self.tx }
    }
}

impl<BlockT, TxT, HardforkT> AsEnvMut for Env<BlockT, TxT, HardforkT> {
    type BlockT = BlockT;
    type HardforkT = HardforkT;
    type TxT = TxT;

    fn as_env_mut(&mut self) -> EnvMut<'_, BlockT, TxT, HardforkT> {
        EnvMut {
            block: &mut self.block,
            cfg: &mut self.cfg,
            tx: &mut self.tx,
        }
    }
}

impl<DB, J, C, BlockT, TxT, HardforkT> AsEnvMut for Context<BlockT, TxT, CfgEnv<HardforkT>, DB, J, C>
where
    DB: Database,
    J: JournalTr<Database = DB>,
{
    type BlockT = BlockT;
    type HardforkT = HardforkT;
    type TxT = TxT;

    fn as_env_mut(&mut self) -> EnvMut<'_, BlockT, TxT, HardforkT> {
        EnvMut { block: &mut self.block, cfg: &mut self.cfg, tx: &mut self.tx }
    }
}

pub trait ContextExt {
    type DB: Database;
    type BlockT;
    type HardforkT;
    type TxT;

    #[allow(clippy::type_complexity)]
    fn as_db_env_and_journal(
        &mut self,
    ) -> (&mut Self::DB, &mut JournalInner<JournalEntry>, EnvMut<'_, Self::BlockT, Self::TxT, Self::HardforkT>);
}

impl<DB, C, BlockT, TxT, HardforkT> ContextExt for Context<BlockT, TxT, CfgEnv<HardforkT>, DB, Journal<DB, JournalEntry>, C>
where
    DB: Database,
{
    type DB = DB;
    type BlockT = BlockT;
    type HardforkT = HardforkT;
    type TxT = TxT;

    fn as_db_env_and_journal(
        &mut self,
    ) -> (&mut Self::DB, &mut JournalInner<JournalEntry>, EnvMut<'_, BlockT, TxT, HardforkT>) {
        (
            &mut self.journaled_state.database,
            &mut self.journaled_state.inner,
            EnvMut { block: &mut self.block, cfg: &mut self.cfg, tx: &mut self.tx },
        )
    }
}
