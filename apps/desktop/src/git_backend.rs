//! Composition root for `gix` (default) and system Git (fallback).

use std::{collections::HashMap, path::Path, sync::atomic::AtomicBool};

use app_core::{
    EngineQuery, GitBackendError, GitHistoryQuery, GitIndexMutate, GitNetwork, GitObjectQuery,
    GitRefQuery, RepositoryDiscoverer, RepositoryOpenError, query_preferring,
};
use git_cli::GitExecutable;
use git_domain::{
    CommitRequest, GitPath, HistoryPage, HistoryRequest, LoadedDiff, RefSnapshot,
    RepositoryLocation, TreeEntry, WorktreeRepository, WorktreeStatus,
};
use git_gix::GixGit;
pub(crate) use git_gix::uses_http_url;

fn system_git() -> Result<GitExecutable, RepositoryOpenError> {
    GitExecutable::discover().map_err(|_| RepositoryOpenError::DiscoveryFailed)
}

fn system_git_query() -> Result<GitExecutable, GitBackendError> {
    GitExecutable::discover().map_err(|error| GitBackendError::from_message(error.to_string()))
}

/// Discovers a repository with `gix`, then system Git if needed.
pub(crate) fn discover_repository(
    path: &Path,
    use_system_git: bool,
) -> EngineQuery<RepositoryLocation, RepositoryOpenError> {
    query_preferring(
        use_system_git,
        || GixGit.discover_repository(path),
        || system_git()?.discover_repository(path),
    )
}

/// Opens a working tree, mapping bare repos to [`RepositoryOpenError::BareRepository`].
pub(crate) fn open_worktree(
    path: &Path,
    use_system_git: bool,
) -> EngineQuery<WorktreeRepository, RepositoryOpenError> {
    let query = discover_repository(path, use_system_git);
    EngineQuery {
        fallback_reason: query.fallback_reason,
        result: query.result.and_then(|location| match location {
            RepositoryLocation::Worktree(repository) => Ok(repository),
            RepositoryLocation::Bare { git_dir } => {
                Err(RepositoryOpenError::BareRepository(git_dir))
            }
        }),
    }
}

/// Loads refs with `gix`, then system Git if needed.
pub(crate) fn ref_snapshot(
    repository: &WorktreeRepository,
    use_system_git: bool,
) -> EngineQuery<RefSnapshot, GitBackendError> {
    query_preferring(
        use_system_git,
        || GixGit.ref_snapshot(repository),
        || GitRefQuery::ref_snapshot(&system_git_query()?, repository),
    )
}

/// Loads working-copy status with `gix`, then system Git if needed.
pub(crate) fn worktree_status(
    repository: &WorktreeRepository,
    use_system_git: bool,
    include_ignored: bool,
) -> EngineQuery<WorktreeStatus, GitBackendError> {
    query_preferring(
        use_system_git,
        || GixGit.worktree_status(repository, include_ignored),
        || GitRefQuery::worktree_status(&system_git_query()?, repository, include_ignored),
    )
}

pub(crate) fn history_page(
    repository: &WorktreeRepository,
    use_system_git: bool,
    request: &HistoryRequest,
) -> EngineQuery<HistoryPage, GitBackendError> {
    query_preferring(
        use_system_git,
        || GixGit.history_page(repository, request),
        || GitHistoryQuery::history_page(&system_git_query()?, repository, request),
    )
}

pub(crate) fn tree_entries(
    repository: &WorktreeRepository,
    use_system_git: bool,
    oid: &str,
    path: &GitPath,
) -> EngineQuery<Vec<TreeEntry>, GitBackendError> {
    query_preferring(
        use_system_git,
        || GixGit.tree_entries(repository, oid, path),
        || GitObjectQuery::tree_entries(&system_git_query()?, repository, oid, path),
    )
}

pub(crate) fn file_at_revision(
    repository: &WorktreeRepository,
    use_system_git: bool,
    oid: &str,
    path: &GitPath,
) -> EngineQuery<Vec<u8>, GitBackendError> {
    query_preferring(
        use_system_git,
        || GixGit.file_at_revision(repository, oid, path),
        || GitObjectQuery::file_at_revision(&system_git_query()?, repository, oid, path),
    )
}

pub(crate) fn file_diff_with_limit(
    repository: &WorktreeRepository,
    use_system_git: bool,
    path: &GitPath,
    staged: bool,
    limit: usize,
) -> EngineQuery<LoadedDiff, GitBackendError> {
    query_preferring(
        use_system_git,
        || GixGit.file_diff_with_limit(repository, path, staged, limit),
        || {
            GitObjectQuery::file_diff_with_limit(
                &system_git_query()?,
                repository,
                path,
                staged,
                limit,
            )
        },
    )
}

pub(crate) fn commit_diff(
    repository: &WorktreeRepository,
    use_system_git: bool,
    oid: &str,
) -> EngineQuery<LoadedDiff, GitBackendError> {
    query_preferring(
        use_system_git,
        || GixGit.commit_diff(repository, oid),
        || GitObjectQuery::commit_diff(&system_git_query()?, repository, oid),
    )
}

pub(crate) fn diff_refs(
    repository: &WorktreeRepository,
    use_system_git: bool,
    left: &str,
    right: &str,
) -> EngineQuery<LoadedDiff, GitBackendError> {
    query_preferring(
        use_system_git,
        || GixGit.diff_refs(repository, left, right),
        || GitObjectQuery::diff_refs(&system_git_query()?, repository, left, right),
    )
}

pub(crate) fn diff_numstat(
    repository: &WorktreeRepository,
    use_system_git: bool,
) -> EngineQuery<HashMap<GitPath, (u64, u64)>, GitBackendError> {
    query_preferring(
        use_system_git,
        || GixGit.diff_numstat(repository),
        || GitObjectQuery::diff_numstat(&system_git_query()?, repository),
    )
}

pub(crate) fn stage_paths(
    repository: &WorktreeRepository,
    use_system_git: bool,
    paths: &[GitPath],
) -> EngineQuery<(), GitBackendError> {
    query_preferring(
        use_system_git,
        || GixGit.stage_paths(repository, paths),
        || GitIndexMutate::stage_paths(&system_git_query()?, repository, paths),
    )
}

pub(crate) fn unstage_paths(
    repository: &WorktreeRepository,
    use_system_git: bool,
    paths: &[GitPath],
) -> EngineQuery<(), GitBackendError> {
    query_preferring(
        use_system_git,
        || GixGit.unstage_paths(repository, paths),
        || GitIndexMutate::unstage_paths(&system_git_query()?, repository, paths),
    )
}

pub(crate) fn stage_all(
    repository: &WorktreeRepository,
    use_system_git: bool,
) -> EngineQuery<(), GitBackendError> {
    query_preferring(
        use_system_git,
        || GixGit.stage_all(repository),
        || GitIndexMutate::stage_all(&system_git_query()?, repository),
    )
}

pub(crate) fn unstage_all(
    repository: &WorktreeRepository,
    use_system_git: bool,
) -> EngineQuery<(), GitBackendError> {
    query_preferring(
        use_system_git,
        || GixGit.unstage_all(repository),
        || GitIndexMutate::unstage_all(&system_git_query()?, repository),
    )
}

pub(crate) fn commit(
    repository: &WorktreeRepository,
    use_system_git: bool,
    request: &CommitRequest,
) -> EngineQuery<(), GitBackendError> {
    query_preferring(
        use_system_git,
        || GixGit.commit(repository, request),
        || GitIndexMutate::commit(&system_git_query()?, repository, request),
    )
}

pub(crate) fn fetch_remote(
    repository: &WorktreeRepository,
    use_system_git: bool,
    remote: &str,
    interrupt: &AtomicBool,
) -> EngineQuery<(), GitBackendError> {
    query_preferring(
        use_system_git,
        || GixGit.fetch_remote(repository, remote, interrupt),
        || GitNetwork::fetch_remote(&system_git_query()?, repository, remote, interrupt),
    )
}

pub(crate) fn clone_repository(
    source: &str,
    destination: &Path,
    use_system_git: bool,
    interrupt: &AtomicBool,
) -> EngineQuery<(), GitBackendError> {
    query_preferring(
        use_system_git || !uses_http_url(source),
        || GixGit.clone_repository(source, destination, interrupt),
        || GitNetwork::clone_repository(&system_git_query()?, source, destination, interrupt),
    )
}
