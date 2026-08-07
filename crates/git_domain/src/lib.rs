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

/// A repository-relative path exactly as reported by Git.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct GitPath(pub Vec<u8>);

/// The branch information emitted by `git status --porcelain=v2 --branch`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BranchStatus {
    pub oid: Option<Vec<u8>>,
    pub head: HeadStatus,
    pub upstream: Option<GitPath>,
    pub ahead: u32,
    pub behind: u32,
}

impl Default for BranchStatus {
    fn default() -> Self {
        Self {
            oid: None,
            head: HeadStatus::Unknown,
            upstream: None,
            ahead: 0,
            behind: 0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HeadStatus {
    Branch(GitPath),
    Detached,
    Unborn,
    Unknown,
}

/// The complete non-mutating working-copy state.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WorktreeStatus {
    pub branch: BranchStatus,
    pub stash_count: u32,
    pub entries: Vec<StatusEntry>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileStatus(pub [u8; 2]);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SubmoduleState {
    NotSubmodule,
    Changed {
        commit: bool,
        modified: bool,
        untracked: bool,
    },
    Unknown(Vec<u8>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenameKind {
    Rename,
    Copy,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StatusEntry {
    Ordinary {
        status: FileStatus,
        submodule: SubmoduleState,
        path: GitPath,
    },
    Renamed {
        status: FileStatus,
        submodule: SubmoduleState,
        kind: RenameKind,
        score: u8,
        path: GitPath,
        source_path: GitPath,
    },
    Unmerged {
        status: FileStatus,
        submodule: SubmoduleState,
        path: GitPath,
    },
    Untracked(GitPath),
    Ignored(GitPath),
}

/// Parsed unified diff output for one or more files.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UnifiedDiff {
    pub files: Vec<DiffFile>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DiffFile {
    pub old_path: Option<GitPath>,
    pub new_path: Option<GitPath>,
    pub binary: bool,
    pub hunks: Vec<DiffHunk>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiffHunk {
    pub header: Vec<u8>,
    pub lines: Vec<DiffLine>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiffLine {
    pub kind: DiffLineKind,
    pub content: Vec<u8>,
    pub missing_final_newline: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiffLineKind {
    Context,
    Addition,
    Removal,
}
