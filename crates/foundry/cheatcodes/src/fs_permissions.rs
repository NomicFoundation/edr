//! Support for controlling fs access

use std::{
    fmt,
    path::{Path, PathBuf},
};

/// Configures file system access
///
/// E.g. for cheat codes (`vm.writeFile`)
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FsPermissions {
    /// what kind of access is allowed
    pub permissions: Vec<PathPermission>,
}

// === impl FsPermissions ===

impl FsPermissions {
    /// Creates anew instance with the given `permissions`
    pub fn new(permissions: impl IntoIterator<Item = PathPermission>) -> Self {
        Self {
            permissions: permissions.into_iter().collect(),
        }
    }

    /// Adds a new permission
    pub fn add(&mut self, permission: PathPermission) {
        self.permissions.push(permission);
    }

    /// Returns true if access to the specified path is allowed with the
    /// specified.
    ///
    /// This first checks permission, and only if it is granted, whether the
    /// path is allowed.
    ///
    /// We only allow paths that are inside  allowed paths.
    ///
    /// Caution: This should be called with normalized paths if the
    /// `allowed_paths` are also normalized.
    pub fn is_path_allowed(&self, path: &Path, kind: FsAccessKind) -> bool {
        self.find_permission(path)
            .map(|perm| perm.is_granted(kind))
            .unwrap_or_default()
    }

    /// Returns the permission for the matching path.
    ///
    /// For file permissions, this will return the exact match.
    ///
    /// For directory permissions, this will return the longest matching path
    /// with resolved sym links, e.g. if we have the following permissions:
    ///
    /// `./out` = `read`
    /// `./out/contracts` = `read-write`
    ///
    /// And we check for `./out/contracts/MyContract.sol` we will get
    /// `read-write` as permission.
    pub fn find_permission(&self, path: &Path) -> Option<FsAccessPermission> {
        let mut permission: Option<&PathPermission> = None;
        for perm in &self.permissions {
            let permission_path = dunce::canonicalize(&perm.path).unwrap_or(perm.path.clone());
            if perm.access == FsAccessPermission::ReadFile
                || perm.access == FsAccessPermission::WriteFile
                || perm.access == FsAccessPermission::ReadWriteFile
            {
                // file permission, check exact match
                if path == permission_path {
                    return Some(perm.access);
                }
            } else if path.starts_with(permission_path) {
                // directory permission, check prefix
                if let Some(active_perm) = permission.as_ref() {
                    // the longest path takes precedence
                    if perm.path < active_perm.path {
                        continue;
                    }
                }
                permission = Some(perm);
            }
        }

        permission.map(|perm| perm.access)
    }

    /// Updates all `allowed_paths` and joins ([`Path::join`]) the `root` with
    /// all entries
    pub fn join_all(&mut self, root: impl AsRef<Path>) {
        let root = root.as_ref();
        self.permissions.iter_mut().for_each(|perm| {
            perm.path = root.join(&perm.path);
        });
    }

    /// Same as [`Self::join_all`] but consumes the type
    pub fn joined(mut self, root: impl AsRef<Path>) -> Self {
        self.join_all(root);
        self
    }

    /// Removes all existing permissions for the given path
    pub fn remove(&mut self, path: impl AsRef<Path>) {
        let path = path.as_ref();
        self.permissions
            .retain(|permission| permission.path != path);
    }

    /// Returns true if no permissions are configured
    pub fn is_empty(&self) -> bool {
        self.permissions.is_empty()
    }

    /// Returns the number of configured permissions
    pub fn len(&self) -> usize {
        self.permissions.len()
    }
}

/// Represents an access permission to a single path
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PathPermission {
    /// Permission level to access the `path`
    pub access: FsAccessPermission,
    /// The targeted path guarded by the permission
    pub path: PathBuf,
}

// === impl PathPermission ===

impl PathPermission {
    /// Returns a new permission for the path and the given access
    pub fn new(path: impl Into<PathBuf>, access: FsAccessPermission) -> Self {
        Self {
            path: path.into(),
            access,
        }
    }

    /// Returns a new read-only permission for the file
    pub fn read_file(path: impl Into<PathBuf>) -> Self {
        Self::new(path, FsAccessPermission::ReadFile)
    }

    /// Returns a new read-write permission for the file
    pub fn read_write_file(path: impl Into<PathBuf>) -> Self {
        Self::new(path, FsAccessPermission::ReadWriteFile)
    }

    /// Returns a new write-only permission for the file
    pub fn write_file(path: impl Into<PathBuf>) -> Self {
        Self::new(path, FsAccessPermission::WriteFile)
    }

    /// Returns a new read-only permission for the directory
    pub fn read_directory(path: impl Into<PathBuf>) -> Self {
        Self::new(path, FsAccessPermission::ReadDirectory)
    }

    /// Returns a new read-write permission for the directory
    pub fn read_write_directory(path: impl Into<PathBuf>) -> Self {
        Self::new(path, FsAccessPermission::DangerouslyReadWriteDirectory)
    }

    /// Returns a new write-only permission for the directory
    pub fn write_directory(path: impl Into<PathBuf>) -> Self {
        Self::new(path, FsAccessPermission::DangerouslyWriteDirectory)
    }

    /// Returns a non permission for the path
    pub fn none(path: impl Into<PathBuf>) -> Self {
        Self::new(path, FsAccessPermission::None)
    }

    /// Returns true if the access is allowed
    pub fn is_granted(&self, kind: FsAccessKind) -> bool {
        self.access.is_granted(kind)
    }
}

/// Represents the operation on the fs
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FsAccessKind {
    /// read from fs (`vm.readFile`)
    Read,
    /// write to fs (`vm.writeFile`)
    Write,
}

impl fmt::Display for FsAccessKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FsAccessKind::Read => f.write_str("read"),
            FsAccessKind::Write => f.write_str("write"),
        }
    }
}

/**
 * Determines the level of file system access for the given path.
 *
 * Exact path matching is used for file permissions. Prefix matching is used
 * for directory permissions.
 *
 * Giving write access to configuration files, source files or executables
 * in a project is considered dangerous, because it can be used by malicious
 * Solidity dependencies to escape the EVM sandbox. It is therefore
 * recommended to give write access to specific safe files only. If write
 * access to a directory is needed, please make sure that it doesn't contain
 * configuration files, source files or executables neither in the top level
 * directory, nor in any subdirectories.
 */
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FsAccessPermission {
    /// FS access is _not_ allowed
    #[default]
    None,
    /// Allows reading and writing the file
    ReadWriteFile,
    /// Only allows reading the file
    ReadFile,
    /// Only allows writing the file
    WriteFile,
    /// Allows reading and writing all files in the directory and its
    /// subdirectories
    DangerouslyReadWriteDirectory,
    /// Allows reading all files in the directory and its subdirectories
    ReadDirectory,
    /// Allows writing all files in the directory and its subdirectories
    DangerouslyWriteDirectory,
}

// === impl FsAccessPermission ===

impl FsAccessPermission {
    /// Returns true if the access is allowed
    pub fn is_granted(&self, kind: FsAccessKind) -> bool {
        #[allow(clippy::match_same_arms)]
        match (self, kind) {
            (FsAccessPermission::ReadWriteFile, _) => true,
            (FsAccessPermission::None, _) => false,
            (FsAccessPermission::ReadFile, FsAccessKind::Read) => true,
            (FsAccessPermission::WriteFile, FsAccessKind::Write) => true,
            (FsAccessPermission::DangerouslyReadWriteDirectory, _) => true,
            (FsAccessPermission::ReadDirectory, FsAccessKind::Read) => true,
            (FsAccessPermission::DangerouslyWriteDirectory, FsAccessKind::Write) => true,
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_permissions() {
        let permissions = FsPermissions::new(vec![
            PathPermission::read_file("./out/contracts/ReadContract.sol"),
            PathPermission::read_write_file("./out/contracts/ReadWriteContract.sol"),
        ]);

        assert!(permissions.is_path_allowed(
            Path::new("./out/contracts/ReadContract.sol"),
            FsAccessKind::Read
        ));
        assert!(permissions.is_path_allowed(
            Path::new("./out/contracts/ReadWriteContract.sol"),
            FsAccessKind::Write
        ));
        assert!(
            !permissions.is_path_allowed(
                Path::new("./out/contracts/NoPermissionContract.sol"),
                FsAccessKind::Write
            ) && !permissions.is_path_allowed(
                Path::new("./out/contracts/NoPermissionContract.sol"),
                FsAccessKind::Read
            )
        );
    }

    #[test]
    fn directory_permissions() {
        let permissions = FsPermissions::new(vec![
            PathPermission::read_directory("./out/contracts"),
            PathPermission::read_write_directory("./out/contracts/readwrite/"),
        ]);

        assert!(permissions.is_path_allowed(Path::new("./out/contracts"), FsAccessKind::Read));
        assert!(!permissions.is_path_allowed(Path::new("./out/contracts"), FsAccessKind::Write));

        assert!(
            permissions.is_path_allowed(Path::new("./out/contracts/readwrite"), FsAccessKind::Read)
        );
        assert!(permissions
            .is_path_allowed(Path::new("./out/contracts/readwrite"), FsAccessKind::Write));

        assert!(!permissions.is_path_allowed(Path::new("./out"), FsAccessKind::Read));
        assert!(!permissions.is_path_allowed(Path::new("./out"), FsAccessKind::Write));
    }

    #[test]
    fn file_and_directory_permissions() {
        let permissions = FsPermissions::new(vec![
            PathPermission::read_directory("./out"),
            PathPermission::write_file("./out/WriteContract.sol"),
        ]);

        assert!(permissions.is_path_allowed(Path::new("./out"), FsAccessKind::Read));
        assert!(
            permissions.is_path_allowed(Path::new("./out/WriteContract.sol"), FsAccessKind::Write)
        );
        // Inherited read from directory
        assert!(
            permissions.is_path_allowed(Path::new("./out/ReadContract.sol"), FsAccessKind::Read)
        );
        // No permission for writing
        assert!(
            !permissions.is_path_allowed(Path::new("./out/ReadContract.sol"), FsAccessKind::Write)
        );
    }

    #[test]
    fn nested_permissions() {
        let permissions = FsPermissions::new(vec![
            PathPermission::read_directory("./"),
            PathPermission::write_directory("./out"),
            PathPermission::read_write_directory("./out/contracts"),
        ]);

        assert!(permissions.is_path_allowed(
            Path::new("./out/contracts/MyContract.sol"),
            FsAccessKind::Write
        ));
        assert!(permissions.is_path_allowed(
            Path::new("./out/contracts/MyContract.sol"),
            FsAccessKind::Read
        ));
        assert!(permissions.is_path_allowed(Path::new("./out/MyContract.sol"), FsAccessKind::Write));
        assert!(!permissions.is_path_allowed(Path::new("./out/MyContract.sol"), FsAccessKind::Read));
    }
}
