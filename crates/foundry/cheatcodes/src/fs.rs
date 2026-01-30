//! Implementations of [`Filesystem`](spec::Group::Filesystem) cheatcodes.

use std::{
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::Command,
    sync::mpsc,
    thread,
    time::{SystemTime, UNIX_EPOCH},
};

use alloy_dyn_abi::DynSolType;
use alloy_primitives::{hex, map::Entry, Bytes, U256};
use alloy_sol_types::SolValue;
use dialoguer::{Input, Password};
use edr_artifact::ArtifactId;
use edr_common::fs;
use foundry_evm_core::{
    backend::CheatcodeBackend,
    evm_context::{
        BlockEnvTr, ChainContextTr, EvmBuilderTrait, HardforkTr, TransactionEnvTr,
        TransactionErrorTrait,
    },
};
use revm::context::result::HaltReasonTr;
use semver::Version;
use walkdir::WalkDir;

use super::string::parse;
use crate::{
    impl_is_pure_false, Cheatcode, Cheatcodes, FsAccessKind, Result,
    Vm::{
        closeFileCall, copyFileCall, createDirCall, existsCall, ffiCall, fsMetadataCall,
        getCodeCall, getDeployedCodeCall, isDirCall, isFileCall, projectRootCall,
        promptAddressCall, promptCall, promptSecretCall, promptSecretUintCall, promptUintCall,
        readDir_0Call, readDir_1Call, readDir_2Call, readFileBinaryCall, readFileCall,
        readLineCall, readLinkCall, removeDirCall, removeFileCall, tryFfiCall, unixTimeCall,
        writeFileBinaryCall, writeFileCall, writeLineCall, DirEntry, FfiResult, FsMetadata,
    },
};

impl_is_pure_false!(existsCall);
impl Cheatcode for existsCall {
    fn apply<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        TransactionErrorT: TransactionErrorTrait,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
        >,
    >(
        &self,
        state: &mut Cheatcodes<
            BlockT,
            TxT,
            ChainContextT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
        >,
    ) -> Result {
        let Self { path } = self;
        let path = state.config.ensure_path_allowed(path, FsAccessKind::Read)?;
        Ok(path.exists().abi_encode())
    }
}

impl_is_pure_false!(fsMetadataCall);
impl Cheatcode for fsMetadataCall {
    fn apply<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        TransactionErrorT: TransactionErrorTrait,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
        >,
    >(
        &self,
        state: &mut Cheatcodes<
            BlockT,
            TxT,
            ChainContextT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
        >,
    ) -> Result {
        let Self { path } = self;
        let path = state.config.ensure_path_allowed(path, FsAccessKind::Read)?;

        let metadata = path.metadata()?;

        // These fields not available on all platforms; default to 0
        let [modified, accessed, created] =
            [metadata.modified(), metadata.accessed(), metadata.created()].map(|time| {
                time.unwrap_or(UNIX_EPOCH)
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs()
            });

        Ok(FsMetadata {
            isDir: metadata.is_dir(),
            isSymlink: metadata.is_symlink(),
            length: U256::from(metadata.len()),
            readOnly: metadata.permissions().readonly(),
            modified: U256::from(modified),
            accessed: U256::from(accessed),
            created: U256::from(created),
        }
        .abi_encode())
    }
}

impl_is_pure_false!(isDirCall);
impl Cheatcode for isDirCall {
    fn apply<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        TransactionErrorT: TransactionErrorTrait,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
        >,
    >(
        &self,
        state: &mut Cheatcodes<
            BlockT,
            TxT,
            ChainContextT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
        >,
    ) -> Result {
        let Self { path } = self;
        let path = state.config.ensure_path_allowed(path, FsAccessKind::Read)?;
        Ok(path.is_dir().abi_encode())
    }
}

impl_is_pure_false!(isFileCall);
impl Cheatcode for isFileCall {
    fn apply<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        TransactionErrorT: TransactionErrorTrait,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
        >,
    >(
        &self,
        state: &mut Cheatcodes<
            BlockT,
            TxT,
            ChainContextT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
        >,
    ) -> Result {
        let Self { path } = self;
        let path = state.config.ensure_path_allowed(path, FsAccessKind::Read)?;
        Ok(path.is_file().abi_encode())
    }
}

impl_is_pure_false!(projectRootCall);
impl Cheatcode for projectRootCall {
    fn apply<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        TransactionErrorT: TransactionErrorTrait,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
        >,
    >(
        &self,
        state: &mut Cheatcodes<
            BlockT,
            TxT,
            ChainContextT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
        >,
    ) -> Result {
        let Self {} = self;
        Ok(state.config.project_root.display().to_string().abi_encode())
    }
}

impl_is_pure_false!(unixTimeCall);
impl Cheatcode for unixTimeCall {
    fn apply<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        TransactionErrorT: TransactionErrorTrait,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
        >,
    >(
        &self,
        _state: &mut Cheatcodes<
            BlockT,
            TxT,
            ChainContextT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
        >,
    ) -> Result {
        let Self {} = self;
        let difference = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| fmt_err!("failed getting Unix timestamp: {e}"))?;
        Ok(difference.as_millis().abi_encode())
    }
}

impl_is_pure_false!(closeFileCall);
impl Cheatcode for closeFileCall {
    fn apply<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        TransactionErrorT: TransactionErrorTrait,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
        >,
    >(
        &self,
        state: &mut Cheatcodes<
            BlockT,
            TxT,
            ChainContextT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
        >,
    ) -> Result {
        let Self { path } = self;
        let path = state.config.ensure_path_allowed(path, FsAccessKind::Read)?;

        state.test_context.opened_read_files.remove(&path);

        Ok(Vec::default())
    }
}

impl_is_pure_false!(copyFileCall);
impl Cheatcode for copyFileCall {
    fn apply<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        TransactionErrorT: TransactionErrorTrait,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
        >,
    >(
        &self,
        state: &mut Cheatcodes<
            BlockT,
            TxT,
            ChainContextT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
        >,
    ) -> Result {
        let Self { from, to } = self;
        let from = state.config.ensure_path_allowed(from, FsAccessKind::Read)?;
        let to = state.config.ensure_path_allowed(to, FsAccessKind::Write)?;

        let n = fs::copy(from, to)?;
        Ok(n.abi_encode())
    }
}

impl_is_pure_false!(createDirCall);
impl Cheatcode for createDirCall {
    fn apply<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        TransactionErrorT: TransactionErrorTrait,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
        >,
    >(
        &self,
        state: &mut Cheatcodes<
            BlockT,
            TxT,
            ChainContextT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
        >,
    ) -> Result {
        let Self { path, recursive } = self;
        let path = state
            .config
            .ensure_path_allowed(path, FsAccessKind::Write)?;
        if *recursive {
            fs::create_dir_all(path)
        } else {
            fs::create_dir(path)
        }?;
        Ok(Vec::default())
    }
}

impl_is_pure_false!(readDir_0Call);
impl Cheatcode for readDir_0Call {
    fn apply<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        TransactionErrorT: TransactionErrorTrait,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
        >,
    >(
        &self,
        state: &mut Cheatcodes<
            BlockT,
            TxT,
            ChainContextT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
        >,
    ) -> Result {
        let Self { path } = self;
        read_dir(state, path.as_ref(), 1, false)
    }
}

impl_is_pure_false!(readDir_1Call);
impl Cheatcode for readDir_1Call {
    fn apply<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        TransactionErrorT: TransactionErrorTrait,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
        >,
    >(
        &self,
        state: &mut Cheatcodes<
            BlockT,
            TxT,
            ChainContextT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
        >,
    ) -> Result {
        let Self { path, maxDepth } = self;
        read_dir(state, path.as_ref(), *maxDepth, false)
    }
}

impl_is_pure_false!(readDir_2Call);
impl Cheatcode for readDir_2Call {
    fn apply<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        TransactionErrorT: TransactionErrorTrait,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
        >,
    >(
        &self,
        state: &mut Cheatcodes<
            BlockT,
            TxT,
            ChainContextT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
        >,
    ) -> Result {
        let Self {
            path,
            maxDepth,
            followLinks,
        } = self;
        read_dir(state, path.as_ref(), *maxDepth, *followLinks)
    }
}

impl_is_pure_false!(readFileCall);
impl Cheatcode for readFileCall {
    fn apply<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        TransactionErrorT: TransactionErrorTrait,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
        >,
    >(
        &self,
        state: &mut Cheatcodes<
            BlockT,
            TxT,
            ChainContextT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
        >,
    ) -> Result {
        let Self { path } = self;
        let path = state.config.ensure_path_allowed(path, FsAccessKind::Read)?;
        Ok(fs::read_to_string(path)?.abi_encode())
    }
}

impl_is_pure_false!(readFileBinaryCall);
impl Cheatcode for readFileBinaryCall {
    fn apply<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        TransactionErrorT: TransactionErrorTrait,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
        >,
    >(
        &self,
        state: &mut Cheatcodes<
            BlockT,
            TxT,
            ChainContextT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
        >,
    ) -> Result {
        let Self { path } = self;
        let path = state.config.ensure_path_allowed(path, FsAccessKind::Read)?;
        Ok(fs::read(path)?.abi_encode())
    }
}

impl_is_pure_false!(readLineCall);
impl Cheatcode for readLineCall {
    fn apply<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        TransactionErrorT: TransactionErrorTrait,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
        >,
    >(
        &self,
        state: &mut Cheatcodes<
            BlockT,
            TxT,
            ChainContextT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
        >,
    ) -> Result {
        let Self { path } = self;
        let path = state.config.ensure_path_allowed(path, FsAccessKind::Read)?;

        // Get reader for previously opened file to continue reading OR initialize new
        // reader
        let reader = match state.test_context.opened_read_files.entry(path.clone()) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => entry.insert(BufReader::new(fs::open(path)?)),
        };

        let mut line: String = String::new();
        reader.read_line(&mut line)?;

        // Remove trailing newline character, preserving others for cases where it may
        // be important
        if line.ends_with('\n') {
            line.pop();
            if line.ends_with('\r') {
                line.pop();
            }
        }

        Ok(line.abi_encode())
    }
}

impl_is_pure_false!(readLinkCall);
impl Cheatcode for readLinkCall {
    fn apply<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        TransactionErrorT: TransactionErrorTrait,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
        >,
    >(
        &self,
        state: &mut Cheatcodes<
            BlockT,
            TxT,
            ChainContextT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
        >,
    ) -> Result {
        let Self { linkPath: path } = self;
        let path = state.config.ensure_path_allowed(path, FsAccessKind::Read)?;
        let target = fs::read_link(path)?;
        Ok(target.display().to_string().abi_encode())
    }
}

impl_is_pure_false!(removeDirCall);
impl Cheatcode for removeDirCall {
    fn apply<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        TransactionErrorT: TransactionErrorTrait,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
        >,
    >(
        &self,
        state: &mut Cheatcodes<
            BlockT,
            TxT,
            ChainContextT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
        >,
    ) -> Result {
        let Self { path, recursive } = self;
        let path = state
            .config
            .ensure_path_allowed(path, FsAccessKind::Write)?;
        if *recursive {
            fs::remove_dir_all(path)
        } else {
            fs::remove_dir(path)
        }?;
        Ok(Vec::default())
    }
}

impl_is_pure_false!(removeFileCall);
impl Cheatcode for removeFileCall {
    fn apply<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        TransactionErrorT: TransactionErrorTrait,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
        >,
    >(
        &self,
        state: &mut Cheatcodes<
            BlockT,
            TxT,
            ChainContextT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
        >,
    ) -> Result {
        let Self { path } = self;
        let path = state
            .config
            .ensure_path_allowed(path, FsAccessKind::Write)?;

        // also remove from the set if opened previously
        state.test_context.opened_read_files.remove(&path);

        if state.fs_commit {
            fs::remove_file(&path)?;
        }

        Ok(Vec::default())
    }
}

impl_is_pure_false!(writeFileCall);
impl Cheatcode for writeFileCall {
    fn apply<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        TransactionErrorT: TransactionErrorTrait,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
        >,
    >(
        &self,
        state: &mut Cheatcodes<
            BlockT,
            TxT,
            ChainContextT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
        >,
    ) -> Result {
        let Self { path, data } = self;
        write_file(state, path.as_ref(), data.as_bytes())
    }
}

impl_is_pure_false!(writeFileBinaryCall);
impl Cheatcode for writeFileBinaryCall {
    fn apply<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        TransactionErrorT: TransactionErrorTrait,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
        >,
    >(
        &self,
        state: &mut Cheatcodes<
            BlockT,
            TxT,
            ChainContextT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
        >,
    ) -> Result {
        let Self { path, data } = self;
        write_file(state, path.as_ref(), data)
    }
}

impl_is_pure_false!(writeLineCall);
impl Cheatcode for writeLineCall {
    fn apply<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        TransactionErrorT: TransactionErrorTrait,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
        >,
    >(
        &self,
        state: &mut Cheatcodes<
            BlockT,
            TxT,
            ChainContextT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
        >,
    ) -> Result {
        let Self { path, data: line } = self;
        let path = state
            .config
            .ensure_path_allowed(path, FsAccessKind::Write)?;

        if state.fs_commit {
            let mut file = std::fs::OpenOptions::new()
                .append(true)
                .create(true)
                .open(path)?;

            writeln!(file, "{line}")?;
        }

        Ok(Vec::default())
    }
}

impl_is_pure_false!(getCodeCall);
impl Cheatcode for getCodeCall {
    fn apply<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        TransactionErrorT: TransactionErrorTrait,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
        >,
    >(
        &self,
        state: &mut Cheatcodes<
            BlockT,
            TxT,
            ChainContextT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
        >,
    ) -> Result {
        let Self { artifactPath: path } = self;
        Ok(get_artifact_code(state, path, false)?.abi_encode())
    }
}

impl_is_pure_false!(getDeployedCodeCall);
impl Cheatcode for getDeployedCodeCall {
    fn apply<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        TransactionErrorT: TransactionErrorTrait,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
        >,
    >(
        &self,
        state: &mut Cheatcodes<
            BlockT,
            TxT,
            ChainContextT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
        >,
    ) -> Result {
        let Self { artifactPath: path } = self;
        Ok(get_artifact_code(state, path, true)?.abi_encode())
    }
}

struct ArtifactIdQuery<'a> {
    file: Option<PathBuf>,
    contract_name: Option<&'a str>,
    version: Option<Version>,
}

impl<'a> ArtifactIdQuery<'a> {
    fn new(path: &'a str) -> Result<Self> {
        let mut parts = path.split(':');

        let mut file = None;
        let mut contract_name = None;
        let mut version = None;

        let path_or_name = parts
            .next()
            .expect("split always returns at least one element");
        if path_or_name.ends_with(".sol") {
            file = Some(PathBuf::from(path_or_name));
            if let Some(name_or_version) = parts.next() {
                if name_or_version.contains('.') {
                    version = Some(name_or_version);
                } else {
                    contract_name = Some(name_or_version);
                    version = parts.next();
                }
            }
        } else {
            contract_name = Some(path_or_name);
            version = parts.next();
        }

        let version = if let Some(version) = version {
            Some(Version::parse(version).map_err(|_err| fmt_err!("Error parsing version"))?)
        } else {
            None
        };

        Ok(Self {
            file,
            contract_name,
            version,
        })
    }

    fn artifact_id_matches(&self, id: &ArtifactId) -> bool {
        // name might be in the form of "Counter.0.8.23"
        let id_name = id
            .name
            .split('.')
            .next()
            .expect("split always returns at least one element");

        if let Some(path) = &self.file
            && !id.source.ends_with(path)
        {
            return false;
        }
        if let Some(name) = self.contract_name
            && id_name != name
        {
            return false;
        }
        if let Some(version) = &self.version
            && (id.version.minor != version.minor
                || id.version.major != version.major
                || id.version.patch != version.patch)
        {
            return false;
        }
        true
    }
}

/// Returns the artifact code from known artifacts
///
/// Can parse following input formats:
/// - `path/to/artifact.json`
/// - `path/to/contract.sol`
/// - `path/to/contract.sol:ContractName`
/// - `path/to/contract.sol:ContractName:0.8.23`
/// - `path/to/contract.sol:0.8.23`
/// - `ContractName`
/// - `ContractName:0.8.23`
fn get_artifact_code<
    BlockT: BlockEnvTr,
    TxT: TransactionEnvTr,
    ChainContextT: ChainContextTr,
    EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
    HaltReasonT: HaltReasonTr,
    HardforkT: HardforkTr,
    TransactionErrorT: TransactionErrorTrait,
>(
    state: &Cheatcodes<
        BlockT,
        TxT,
        ChainContextT,
        EvmBuilderT,
        HaltReasonT,
        HardforkT,
        TransactionErrorT,
    >,
    path: &str,
    deployed: bool,
) -> Result<Bytes> {
    let query = ArtifactIdQuery::new(path)?;

    let filtered = state
        .config
        .available_artifacts
        .iter()
        .filter(|(id, _)| query.artifact_id_matches(id))
        .collect::<Vec<_>>();

    let artifact = match &filtered[..] {
        [] => Err(fmt_err!("no matching artifact found")),
        [artifact] => Ok(*artifact),
        filtered => {
            let mut filtered = filtered.to_vec();
            // If we know the current script/test contract solc version, try to filter by it
            state
                .config
                .running_artifact
                .as_ref()
                .and_then(|running| {
                    // Firstly filter by version
                    filtered.retain(|(id, _)| id.version == running.version);

                    if filtered.len() == 1 {
                        filtered.first().copied()
                    } else {
                        None
                    }
                })
                .ok_or_else(|| fmt_err!("multiple matching artifacts found"))
        }
    }?
    .1;

    let maybe_bytecode = if deployed {
        artifact.deployed_bytecode.clone()
    } else {
        artifact.bytecode.clone()
    };

    maybe_bytecode.ok_or_else(|| fmt_err!("No bytecode for contract. Is it abstract or unlinked?"))
}

impl_is_pure_false!(ffiCall);
impl Cheatcode for ffiCall {
    fn apply<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        TransactionErrorT: TransactionErrorTrait,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
        >,
    >(
        &self,
        state: &mut Cheatcodes<
            BlockT,
            TxT,
            ChainContextT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
        >,
    ) -> Result {
        let Self {
            commandInput: input,
        } = self;

        let output = ffi(state, input)?;
        // TODO: check exit code?
        if !output.stderr.is_empty() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            error!(target: "cheatcodes", ?input, ?stderr, "non-empty stderr");
        }
        // we already hex-decoded the stdout in `ffi`
        Ok(output.stdout.abi_encode())
    }
}

impl_is_pure_false!(tryFfiCall);
impl Cheatcode for tryFfiCall {
    fn apply<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        TransactionErrorT: TransactionErrorTrait,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
        >,
    >(
        &self,
        state: &mut Cheatcodes<
            BlockT,
            TxT,
            ChainContextT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
        >,
    ) -> Result {
        let Self {
            commandInput: input,
        } = self;
        ffi(state, input).map(|res| res.abi_encode())
    }
}

impl_is_pure_false!(promptCall);
impl Cheatcode for promptCall {
    fn apply<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        TransactionErrorT: TransactionErrorTrait,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
        >,
    >(
        &self,
        state: &mut Cheatcodes<
            BlockT,
            TxT,
            ChainContextT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
        >,
    ) -> Result {
        let Self { promptText: text } = self;
        prompt(state, text, prompt_input).map(|res| res.abi_encode())
    }
}

impl_is_pure_false!(promptSecretCall);
impl Cheatcode for promptSecretCall {
    fn apply<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        TransactionErrorT: TransactionErrorTrait,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
        >,
    >(
        &self,
        state: &mut Cheatcodes<
            BlockT,
            TxT,
            ChainContextT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
        >,
    ) -> Result {
        let Self { promptText: text } = self;
        prompt(state, text, prompt_password).map(|res| res.abi_encode())
    }
}

impl_is_pure_false!(promptSecretUintCall);
impl Cheatcode for promptSecretUintCall {
    fn apply<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        TransactionErrorT: TransactionErrorTrait,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
        >,
    >(
        &self,
        state: &mut Cheatcodes<
            BlockT,
            TxT,
            ChainContextT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
        >,
    ) -> Result {
        let Self { promptText: text } = self;
        parse(
            &prompt(state, text, prompt_password)?,
            &DynSolType::Uint(256),
        )
    }
}

impl_is_pure_false!(promptAddressCall);
impl Cheatcode for promptAddressCall {
    fn apply<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        TransactionErrorT: TransactionErrorTrait,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
        >,
    >(
        &self,
        state: &mut Cheatcodes<
            BlockT,
            TxT,
            ChainContextT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
        >,
    ) -> Result {
        let Self { promptText: text } = self;
        parse(&prompt(state, text, prompt_input)?, &DynSolType::Address)
    }
}

impl_is_pure_false!(promptUintCall);
impl Cheatcode for promptUintCall {
    fn apply<
        BlockT: BlockEnvTr,
        TxT: TransactionEnvTr,
        EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
        HaltReasonT: HaltReasonTr,
        HardforkT: HardforkTr,
        TransactionErrorT: TransactionErrorTrait,
        ChainContextT: ChainContextTr,
        DatabaseT: CheatcodeBackend<
            BlockT,
            TxT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
            ChainContextT,
        >,
    >(
        &self,
        state: &mut Cheatcodes<
            BlockT,
            TxT,
            ChainContextT,
            EvmBuilderT,
            HaltReasonT,
            HardforkT,
            TransactionErrorT,
        >,
    ) -> Result {
        let Self { promptText: text } = self;
        parse(&prompt(state, text, prompt_input)?, &DynSolType::Uint(256))
    }
}

pub(super) fn write_file<
    BlockT: BlockEnvTr,
    TxT: TransactionEnvTr,
    ChainContextT: ChainContextTr,
    EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
    HaltReasonT: HaltReasonTr,
    HardforkT: HardforkTr,
    TransactionErrorT: TransactionErrorTrait,
>(
    state: &Cheatcodes<
        BlockT,
        TxT,
        ChainContextT,
        EvmBuilderT,
        HaltReasonT,
        HardforkT,
        TransactionErrorT,
    >,
    path: &Path,
    contents: &[u8],
) -> Result {
    let path = state
        .config
        .ensure_path_allowed(path, FsAccessKind::Write)?;

    if state.fs_commit {
        fs::write(path, contents)?;
    }

    Ok(Vec::default())
}

fn read_dir<
    BlockT: BlockEnvTr,
    TxT: TransactionEnvTr,
    ChainContextT: ChainContextTr,
    EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
    HaltReasonT: HaltReasonTr,
    HardforkT: HardforkTr,
    TransactionErrorT: TransactionErrorTrait,
>(
    state: &Cheatcodes<
        BlockT,
        TxT,
        ChainContextT,
        EvmBuilderT,
        HaltReasonT,
        HardforkT,
        TransactionErrorT,
    >,
    path: &Path,
    max_depth: u64,
    follow_links: bool,
) -> Result {
    let root = state.config.ensure_path_allowed(path, FsAccessKind::Read)?;
    let paths: Vec<DirEntry> = WalkDir::new(root)
        .min_depth(1)
        .max_depth(max_depth.try_into().unwrap_or(usize::MAX))
        .follow_links(follow_links)
        .contents_first(false)
        .same_file_system(true)
        .sort_by_file_name()
        .into_iter()
        .map(|entry| match entry {
            Ok(entry) => DirEntry {
                errorMessage: String::new(),
                path: entry.path().display().to_string(),
                depth: entry.depth() as u64,
                isDir: entry.file_type().is_dir(),
                isSymlink: entry.path_is_symlink(),
            },
            Err(e) => DirEntry {
                errorMessage: e.to_string(),
                path: e
                    .path()
                    .map(|p| p.display().to_string())
                    .unwrap_or_default(),
                depth: e.depth() as u64,
                isDir: false,
                isSymlink: false,
            },
        })
        .collect();
    Ok(paths.abi_encode())
}

fn ffi<
    BlockT: BlockEnvTr,
    TxT: TransactionEnvTr,
    ChainContextT: ChainContextTr,
    EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
    HaltReasonT: HaltReasonTr,
    HardforkT: HardforkTr,
    TransactionErrorT: TransactionErrorTrait,
>(
    state: &Cheatcodes<
        BlockT,
        TxT,
        ChainContextT,
        EvmBuilderT,
        HaltReasonT,
        HardforkT,
        TransactionErrorT,
    >,
    input: &[String],
) -> Result<FfiResult> {
    ensure!(
        state.config.ffi,
        "FFI is disabled; add the `--ffi` flag to allow tests to call external commands"
    );
    let first = input
        .first()
        .ok_or_else(|| fmt_err!("can't execute empty command"))?;
    ensure!(!first.is_empty(), "can't execute empty command");
    let mut cmd = Command::new(first);
    if let Some(args) = input.get(1..) {
        cmd.args(args);
    }

    debug!(target: "cheatcodes", ?cmd, "invoking ffi");

    let output = cmd
        .current_dir(&state.config.project_root)
        .output()
        .map_err(|err| fmt_err!("failed to execute command {cmd:?}: {err}"))?;

    // The stdout might be encoded on valid hex, or it might just be a string,
    // so we need to determine which it is to avoid improperly encoding later.
    let trimmed_stdout = String::from_utf8(output.stdout)?;
    let trimmed_stdout = trimmed_stdout.trim();
    let encoded_stdout = if let Ok(hex) = hex::decode(trimmed_stdout) {
        hex
    } else {
        trimmed_stdout.as_bytes().to_vec()
    };
    Ok(FfiResult {
        exitCode: output.status.code().unwrap_or(69),
        stdout: encoded_stdout.into(),
        stderr: output.stderr.into(),
    })
}

fn prompt_input(prompt_text: &str) -> Result<String, dialoguer::Error> {
    Input::new()
        .allow_empty(true)
        .with_prompt(prompt_text)
        .interact_text()
}

fn prompt_password(prompt_text: &str) -> Result<String, dialoguer::Error> {
    Password::new().with_prompt(prompt_text).interact()
}

fn prompt<
    BlockT: BlockEnvTr,
    TxT: TransactionEnvTr,
    ChainContextT: ChainContextTr,
    EvmBuilderT: EvmBuilderTrait<BlockT, ChainContextT, HaltReasonT, HardforkT, TransactionErrorT, TxT>,
    HaltReasonT: HaltReasonTr,
    HardforkT: HardforkTr,
    TransactionErrorT: TransactionErrorTrait,
>(
    state: &Cheatcodes<
        BlockT,
        TxT,
        ChainContextT,
        EvmBuilderT,
        HaltReasonT,
        HardforkT,
        TransactionErrorT,
    >,
    prompt_text: &str,
    input: fn(&str) -> Result<String, dialoguer::Error>,
) -> Result<String> {
    let text_clone = prompt_text.to_string();
    let timeout = state.config.prompt_timeout;
    let (tx, rx) = mpsc::channel();

    thread::spawn(move || {
        let _ = tx.send(input(&text_clone));
    });

    if let Ok(res) = rx.recv_timeout(timeout) {
        res.map_err(|err| {
            println!();
            err.to_string().into()
        })
    } else {
        println!();
        Err("Prompt timed out".into())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use alloy_json_abi::ContractObject;
    use foundry_evm_core::evm_context::L1EvmBuilder;
    use revm::{
        context::{
            result::{HaltReason, InvalidTransaction},
            BlockEnv, TxEnv,
        },
        primitives::hardfork::SpecId,
    };

    use super::*;
    use crate::CheatsConfig;

    fn cheats(
    ) -> Cheatcodes<BlockEnv, TxEnv, (), L1EvmBuilder, HaltReason, SpecId, InvalidTransaction> {
        let config = CheatsConfig {
            ffi: true,
            project_root: PathBuf::from(&env!("CARGO_MANIFEST_DIR")),
            ..Default::default()
        };
        Cheatcodes::new(Arc::new(config))
    }

    #[test]
    fn test_ffi_hex() {
        let msg = b"gm";
        let cheats = cheats();
        let args = ["echo".to_string(), hex::encode(msg)];
        let output = ffi(&cheats, &args).unwrap();
        assert_eq!(output.stdout, Bytes::from(msg));
    }

    #[test]
    fn test_ffi_string() {
        let msg = "gm";
        let cheats = cheats();
        let args = ["echo".to_string(), msg.to_string()];
        let output = ffi(&cheats, &args).unwrap();
        assert_eq!(output.stdout, Bytes::from(msg.as_bytes()));
    }

    #[test]
    fn test_artifact_parsing() {
        let s = include_str!("../../evm/test-data/solc-obj.json");
        let artifact: ContractObject = serde_json::from_str(s).unwrap();
        assert!(artifact.bytecode.is_some());

        let artifact: ContractObject = serde_json::from_str(s).unwrap();
        assert!(artifact.deployed_bytecode.is_some());
    }
}
