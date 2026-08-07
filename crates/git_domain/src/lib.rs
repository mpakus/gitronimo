//! Pure Git domain types. This crate must not depend on UI, process, or platform APIs.

use std::path::PathBuf;

/// The canonical locations that identify an opened working-tree repository.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorktreeRepository {
    pub worktree_root: PathBuf,
    pub git_dir: PathBuf,
}

/// A location selected by the user and classified by Git.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RepositoryLocation {
    Worktree(WorktreeRepository),
    Bare { git_dir: PathBuf },
}
