//! File locking utilities.

use std::{
    fs::{self, File, OpenOptions},
    path::{Path, PathBuf},
};

pub use fd_lock::RwLock;
use foundry_compilers::solc::Solc;

/// Creates a new lock file at the given path.
pub fn new_fd_lock(lock_path: impl AsRef<Path>) -> RwLock<File> {
    fn new_lock(lock_path: &Path) -> RwLock<File> {
        let lock_file = pretty_err(
            lock_path,
            OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(false)
                .open(lock_path),
        );
        RwLock::new(lock_file)
    }
    new_lock(lock_path.as_ref())
}

/// Returns the path to the SVM lock file, creating the SVM directory if it
/// doesn't exist yet.
fn svm_lock_path() -> PathBuf {
    let svm_dir = Solc::svm_home().expect("SVM home directory not found");
    fs::create_dir_all(&svm_dir).expect("failed to create SVM directory");
    svm_dir.join(".lock")
}

/// Creates a file lock at the SVM data directory. This should be used to
/// synchronize solc installations across test crates that run concurrently
/// during `cargo test --workspace`.
pub fn svm_global_lock() -> RwLock<File> {
    new_fd_lock(svm_lock_path())
}

/// Returns `true` if the SVM lock file has been marked as initialized
/// (i.e. solc versions have already been installed).
pub fn is_svm_initialized() -> bool {
    fs::read(svm_lock_path()).unwrap_or_default() == b"1"
}

/// Marks the SVM lock file as initialized by writing `"1"` to it.
pub fn mark_svm_initialized() {
    fs::write(svm_lock_path(), b"1").expect("failed to write SVM lock file");
}

#[track_caller]
fn pretty_err<T, E: std::error::Error>(path: impl AsRef<Path>, res: Result<T, E>) -> T {
    match res {
        Ok(t) => t,
        Err(err) => panic!("{}: {err}", path.as_ref().display()),
    }
}
