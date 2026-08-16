//! Ports for the Git engine (`gix` default, system Git fallback).

use std::{collections::HashMap, fmt, path::Path, sync::atomic::AtomicBool};

use git_domain::{
    CommitRequest, GitPath, HeadStatus, HistoryPage, HistoryRequest, LoadedDiff, RefSnapshot,
    TreeEntry, WorktreeRepository, WorktreeStatus,
};

/// Preferred Git implementation. `Gix` is the 2.0 default.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum GitEngineKind {
    /// gitoxide `gix` library (not the `gix` CLI).
    #[default]
    Gix,
    /// Installed Git executable via `git_cli`.
    SystemGit,
}

impl GitEngineKind {
    /// Settings override: force the installed Git executable.
    #[must_use]
    pub const fn from_use_system_git(use_system_git: bool) -> Self {
        if use_system_git {
            Self::SystemGit
        } else {
            Self::Gix
        }
    }

    /// Whether Settings should show the system-Git override as on.
    #[must_use]
    pub const fn use_system_git(self) -> bool {
        matches!(self, Self::SystemGit)
    }
}

/// Failure from a Git query port. Messages must already be redacted by adapters.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GitBackendError {
    message: String,
}

impl GitBackendError {
    /// Builds an error from adapter text that must not contain secrets.
    #[must_use]
    pub fn from_message(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    /// Redacted adapter message for activity lines.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for GitBackendError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for GitBackendError {}

/// Non-mutating HEAD and ref queries. Implemented by `git_gix` and `git_cli`.
pub trait GitRefQuery {
    /// Classifies `HEAD` as a branch, detached, or unborn.
    ///
    /// # Errors
    /// Returns when the engine cannot read `HEAD`.
    fn head_status(&self, repository: &WorktreeRepository) -> Result<HeadStatus, GitBackendError>;

    /// Resolves the `HEAD` object id.
    ///
    /// # Errors
    /// Returns when `HEAD` is unborn or cannot be read.
    fn head_oid(&self, repository: &WorktreeRepository) -> Result<String, GitBackendError>;

    /// Lists local branches, remote branches, tags, and remotes.
    ///
    /// # Errors
    /// Returns when refs or remote configuration cannot be read.
    fn ref_snapshot(&self, repository: &WorktreeRepository)
    -> Result<RefSnapshot, GitBackendError>;

    /// Index vs `HEAD`, index vs worktree, untracked paths, and optional ignored paths.
    ///
    /// # Errors
    /// Returns when the engine cannot read status, HEAD, or the stash reflog.
    fn worktree_status(
        &self,
        repository: &WorktreeRepository,
        include_ignored: bool,
    ) -> Result<WorktreeStatus, GitBackendError>;
}

/// Bounded commit history. Implemented by `git_gix` and `git_cli`.
pub trait GitHistoryQuery {
    /// Loads one page of history for the requested reference.
    ///
    /// # Errors
    /// Returns when the engine cannot walk revisions or read commit metadata.
    fn history_page(
        &self,
        repository: &WorktreeRepository,
        request: &HistoryRequest,
    ) -> Result<HistoryPage, GitBackendError>;
}

/// Tree, blob, and unified-diff reads. Implemented by `git_gix` and `git_cli`.
pub trait GitObjectQuery {
    /// Lists one directory of a commit's tree. An empty path lists the root.
    ///
    /// # Errors
    /// Returns when the revision is not a commit or tree, or the path is missing.
    fn tree_entries(
        &self,
        repository: &WorktreeRepository,
        oid: &str,
        path: &GitPath,
    ) -> Result<Vec<TreeEntry>, GitBackendError>;

    /// Reads blob bytes at a revision.
    ///
    /// # Errors
    /// Returns when the path is not a blob in that tree.
    fn file_at_revision(
        &self,
        repository: &WorktreeRepository,
        oid: &str,
        path: &GitPath,
    ) -> Result<Vec<u8>, GitBackendError>;

    /// Loads one staged or unstaged file diff with a display-byte limit.
    ///
    /// # Errors
    /// Returns when the engine cannot read the index, HEAD, worktree, or blobs.
    fn file_diff_with_limit(
        &self,
        repository: &WorktreeRepository,
        path: &GitPath,
        staged: bool,
        limit: usize,
    ) -> Result<LoadedDiff, GitBackendError>;

    /// Loads a bounded unified diff for one commit against its first parent.
    ///
    /// # Errors
    /// Returns when the commit cannot be read or diffed.
    fn commit_diff(
        &self,
        repository: &WorktreeRepository,
        oid: &str,
    ) -> Result<LoadedDiff, GitBackendError>;

    /// Loads a bounded two-dot unified diff between two refs.
    ///
    /// # Errors
    /// Returns when either ref cannot be resolved.
    fn diff_refs(
        &self,
        repository: &WorktreeRepository,
        left: &str,
        right: &str,
    ) -> Result<LoadedDiff, GitBackendError>;

    /// Per-path addition/deletion counts from staged and unstaged diffs.
    ///
    /// # Errors
    /// Returns when the engine cannot read the index, HEAD, or worktree.
    fn diff_numstat(
        &self,
        repository: &WorktreeRepository,
    ) -> Result<HashMap<GitPath, (u64, u64)>, GitBackendError>;
}

/// Low-level index and commit mutations. Hunk staging stays on system Git.
pub trait GitIndexMutate {
    /// Stages the supplied repository-relative paths, including deletions.
    ///
    /// # Errors
    /// Returns when the index cannot be written or a path is unsafe.
    fn stage_paths(
        &self,
        repository: &WorktreeRepository,
        paths: &[GitPath],
    ) -> Result<(), GitBackendError>;

    /// Unstages the supplied paths, including in an unborn repository.
    ///
    /// # Errors
    /// Returns when the index cannot be written or a path is unsafe.
    fn unstage_paths(
        &self,
        repository: &WorktreeRepository,
        paths: &[GitPath],
    ) -> Result<(), GitBackendError>;

    /// Stages all working-copy changes, including deletions and untracked files.
    ///
    /// # Errors
    /// Returns when the index cannot be written.
    fn stage_all(&self, repository: &WorktreeRepository) -> Result<(), GitBackendError>;

    /// Unstages every index entry, including in an unborn repository.
    ///
    /// # Errors
    /// Returns when the index cannot be written.
    fn unstage_all(&self, repository: &WorktreeRepository) -> Result<(), GitBackendError>;

    /// Commits the staged index. Does not run hooks or GPG-sign; those stay on system Git.
    ///
    /// # Errors
    /// Returns for an empty subject, missing identity, empty non-amend commit, hooks, or `commit.gpgsign`.
    fn commit(
        &self,
        repository: &WorktreeRepository,
        request: &CommitRequest,
    ) -> Result<(), GitBackendError>;
}

/// HTTPS fetch and clone. SSH and `file://` stay on system Git.
pub trait GitNetwork {
    /// Fetches configured refs from `remote`. Interruptible via `interrupt`.
    ///
    /// # Errors
    /// Returns when the remote is not HTTP(S), credentials fail, or the fetch is interrupted.
    fn fetch_remote(
        &self,
        repository: &WorktreeRepository,
        remote: &str,
        interrupt: &AtomicBool,
    ) -> Result<(), GitBackendError>;

    /// Clones `source` into `destination`. Interruptible via `interrupt`.
    ///
    /// # Errors
    /// Returns when the URL is not HTTP(S), the destination cannot be created, or clone is interrupted.
    fn clone_repository(
        &self,
        source: &str,
        destination: &Path,
        interrupt: &AtomicBool,
    ) -> Result<(), GitBackendError>;
}

/// Outcome of trying `gix` then system Git.
#[must_use]
pub struct EngineQuery<T, E> {
    /// Successful value or the fallback engine's error.
    pub result: Result<T, E>,
    /// Preferred-engine error when fallback ran.
    pub fallback_reason: Option<String>,
}

/// Runs `preferred` unless `force_fallback` is set, then `fallback`.
pub fn query_preferring<T, E>(
    force_fallback: bool,
    preferred: impl FnOnce() -> Result<T, E>,
    fallback: impl FnOnce() -> Result<T, E>,
) -> EngineQuery<T, E>
where
    E: fmt::Display,
{
    if force_fallback {
        return EngineQuery {
            result: fallback(),
            fallback_reason: None,
        };
    }
    match preferred() {
        Ok(value) => EngineQuery {
            result: Ok(value),
            fallback_reason: None,
        },
        Err(error) => EngineQuery {
            fallback_reason: Some(error.to_string()),
            result: fallback(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::{GitEngineKind, query_preferring};

    #[test]
    fn system_git_override_skips_preferred_engine() {
        let query = query_preferring(true, || Err::<u8, &str>("gix should not run"), || Ok(7));
        assert_eq!(query.result, Ok(7));
        assert_eq!(query.fallback_reason, None);
    }

    #[test]
    fn preferred_success_does_not_call_fallback() {
        let query = query_preferring(false, || Ok(3), || Err::<u8, &str>("fallback"));
        assert_eq!(query.result, Ok(3));
        assert_eq!(query.fallback_reason, None);
    }

    #[test]
    fn preferred_error_runs_fallback_and_keeps_reason() {
        let query = query_preferring(false, || Err::<u8, &str>("gix missing"), || Ok(1));
        assert_eq!(query.result, Ok(1));
        assert_eq!(query.fallback_reason.as_deref(), Some("gix missing"));
    }

    #[test]
    fn engine_kind_round_trips_settings_flag() {
        assert!(!GitEngineKind::Gix.use_system_git());
        assert!(GitEngineKind::SystemGit.use_system_git());
        assert_eq!(
            GitEngineKind::from_use_system_git(false),
            GitEngineKind::Gix
        );
        assert_eq!(
            GitEngineKind::from_use_system_git(true),
            GitEngineKind::SystemGit
        );
    }
}
