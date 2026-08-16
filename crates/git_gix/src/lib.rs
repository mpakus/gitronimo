//! gitoxide `gix` adapter. This crate is the only place that imports `gix`.

use std::{
    collections::HashMap,
    os::unix::ffi::OsStringExt,
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
};

use app_core::{
    GitBackendError, GitHistoryQuery, GitIndexMutate, GitNetwork, GitObjectQuery, GitRefQuery,
    RepositoryDiscoverer, RepositoryOpenError,
};
use git_domain::{
    CommitRequest, GitPath, HeadStatus, HistoryPage, HistoryRequest, LoadedDiff,
    MAX_DISPLAY_DIFF_BYTES, NamedRef, RefSnapshot, Remote, RepositoryLocation, TreeEntry,
    WorktreeRepository, WorktreeStatus,
};

mod history;
mod mutate;
mod network;
mod objects;
mod status;

pub use network::uses_http_url;

/// Stateless `gix` backend.
#[derive(Clone, Copy, Debug, Default)]
pub struct GixGit;

impl GixGit {
    fn open(repository: &WorktreeRepository) -> Result<gix::Repository, GitBackendError> {
        gix::open(&repository.worktree_root).map_err(gix_error)
    }
}

impl RepositoryDiscoverer for GixGit {
    fn discover_repository(&self, path: &Path) -> Result<RepositoryLocation, RepositoryOpenError> {
        discover_repository(path)
    }
}

impl GitRefQuery for GixGit {
    fn head_status(&self, repository: &WorktreeRepository) -> Result<HeadStatus, GitBackendError> {
        head_status(&Self::open(repository)?)
    }

    fn head_oid(&self, repository: &WorktreeRepository) -> Result<String, GitBackendError> {
        let repo = Self::open(repository)?;
        let id = repo.head_id().map_err(gix_error)?;
        Ok(id.to_string())
    }

    fn ref_snapshot(
        &self,
        repository: &WorktreeRepository,
    ) -> Result<RefSnapshot, GitBackendError> {
        ref_snapshot(&Self::open(repository)?)
    }

    fn worktree_status(
        &self,
        repository: &WorktreeRepository,
        include_ignored: bool,
    ) -> Result<WorktreeStatus, GitBackendError> {
        status::worktree_status(&Self::open(repository)?, include_ignored)
    }
}

impl GitHistoryQuery for GixGit {
    fn history_page(
        &self,
        repository: &WorktreeRepository,
        request: &HistoryRequest,
    ) -> Result<HistoryPage, GitBackendError> {
        history::history_page(&Self::open(repository)?, request)
    }
}

impl GitObjectQuery for GixGit {
    fn tree_entries(
        &self,
        repository: &WorktreeRepository,
        oid: &str,
        path: &GitPath,
    ) -> Result<Vec<TreeEntry>, GitBackendError> {
        objects::tree_entries(&Self::open(repository)?, oid, path)
    }

    fn file_at_revision(
        &self,
        repository: &WorktreeRepository,
        oid: &str,
        path: &GitPath,
    ) -> Result<Vec<u8>, GitBackendError> {
        objects::file_at_revision(&Self::open(repository)?, oid, path)
    }

    fn file_diff_with_limit(
        &self,
        repository: &WorktreeRepository,
        path: &GitPath,
        staged: bool,
        limit: usize,
    ) -> Result<LoadedDiff, GitBackendError> {
        objects::file_diff_with_limit(&Self::open(repository)?, repository, path, staged, limit)
    }

    fn commit_diff(
        &self,
        repository: &WorktreeRepository,
        oid: &str,
    ) -> Result<LoadedDiff, GitBackendError> {
        objects::commit_diff(&Self::open(repository)?, oid, MAX_DISPLAY_DIFF_BYTES)
    }

    fn diff_refs(
        &self,
        repository: &WorktreeRepository,
        left: &str,
        right: &str,
    ) -> Result<LoadedDiff, GitBackendError> {
        objects::diff_refs(
            &Self::open(repository)?,
            left,
            right,
            MAX_DISPLAY_DIFF_BYTES,
        )
    }

    fn diff_numstat(
        &self,
        repository: &WorktreeRepository,
    ) -> Result<HashMap<GitPath, (u64, u64)>, GitBackendError> {
        objects::diff_numstat(&Self::open(repository)?, repository)
    }
}

impl GitIndexMutate for GixGit {
    fn stage_paths(
        &self,
        repository: &WorktreeRepository,
        paths: &[GitPath],
    ) -> Result<(), GitBackendError> {
        mutate::stage_paths(&Self::open(repository)?, repository, paths)
    }

    fn unstage_paths(
        &self,
        repository: &WorktreeRepository,
        paths: &[GitPath],
    ) -> Result<(), GitBackendError> {
        mutate::unstage_paths(&Self::open(repository)?, repository, paths)
    }

    fn stage_all(&self, repository: &WorktreeRepository) -> Result<(), GitBackendError> {
        mutate::stage_all(&Self::open(repository)?, repository)
    }

    fn unstage_all(&self, repository: &WorktreeRepository) -> Result<(), GitBackendError> {
        mutate::unstage_all(&Self::open(repository)?)
    }

    fn commit(
        &self,
        repository: &WorktreeRepository,
        request: &CommitRequest,
    ) -> Result<(), GitBackendError> {
        mutate::commit(&Self::open(repository)?, request)
    }
}

impl GitNetwork for GixGit {
    fn fetch_remote(
        &self,
        repository: &WorktreeRepository,
        remote: &str,
        interrupt: &AtomicBool,
    ) -> Result<(), GitBackendError> {
        network::fetch_remote(&Self::open(repository)?, remote, interrupt)
    }

    fn clone_repository(
        &self,
        source: &str,
        destination: &Path,
        interrupt: &AtomicBool,
    ) -> Result<(), GitBackendError> {
        network::clone_repository(source, destination, interrupt)
    }
}

fn discover_repository(path: &Path) -> Result<RepositoryLocation, RepositoryOpenError> {
    if !path.is_dir() {
        return Err(RepositoryOpenError::NotDirectory(path.to_path_buf()));
    }
    let repo = gix::discover_with_environment_overrides(path)
        .map_err(|_| RepositoryOpenError::NotRepository(path.to_path_buf()))?;
    let git_dir = canonicalize_dir(repo.git_dir())?;
    if repo.is_bare() {
        return Ok(RepositoryLocation::Bare { git_dir });
    }
    let worktree_root = repo
        .workdir()
        .ok_or(RepositoryOpenError::DiscoveryFailed)
        .and_then(canonicalize_dir)?;
    Ok(RepositoryLocation::Worktree(WorktreeRepository {
        worktree_root,
        git_dir,
    }))
}

fn canonicalize_dir(path: &Path) -> Result<PathBuf, RepositoryOpenError> {
    path.canonicalize()
        .map_err(|_| RepositoryOpenError::DiscoveryFailed)
}

pub(crate) fn head_status(repo: &gix::Repository) -> Result<HeadStatus, GitBackendError> {
    let head = repo.head().map_err(gix_error)?;
    if head.is_unborn() {
        // Porcelain-v2 on current Git prints the unborn branch name, not `(initial)`.
        return Ok(head.referent_name().map_or(HeadStatus::Unborn, |name| {
            HeadStatus::Branch(GitPath(name.shorten().to_vec()))
        }));
    }
    if head.is_detached() {
        return Ok(HeadStatus::Detached);
    }
    let Some(name) = head.referent_name() else {
        return Ok(HeadStatus::Unknown);
    };
    Ok(HeadStatus::Branch(GitPath(name.shorten().to_vec())))
}

fn ref_snapshot(repo: &gix::Repository) -> Result<RefSnapshot, GitBackendError> {
    let platform = repo.references().map_err(gix_error)?;
    let mut snapshot = RefSnapshot::default();
    collect_local_branches(repo, &platform, &mut snapshot)?;
    collect_named_refs(platform.remote_branches().map_err(gix_error)?, |named| {
        snapshot.remote_branches.push(named);
    })?;
    collect_named_refs(platform.tags().map_err(gix_error)?, |named| {
        snapshot.tags.push(named);
    })?;
    collect_remotes(repo, &mut snapshot)?;
    Ok(snapshot)
}

fn collect_local_branches(
    repo: &gix::Repository,
    platform: &gix::reference::iter::Platform<'_>,
    snapshot: &mut RefSnapshot,
) -> Result<(), GitBackendError> {
    for reference in platform.local_branches().map_err(gix_error)? {
        let mut reference = reference.map_err(gix_error)?;
        let full_name = reference.name().to_owned();
        let name = GitPath(full_name.shorten().to_vec());
        let target = object_id(&mut reference)?;
        let (upstream, ahead, behind) = tracking(repo, full_name.as_ref())?;
        snapshot.local_branches.push(NamedRef {
            name,
            target,
            upstream,
            ahead,
            behind,
        });
    }
    Ok(())
}

fn collect_named_refs(
    iter: gix::reference::iter::Iter<'_, '_>,
    mut push: impl FnMut(NamedRef),
) -> Result<(), GitBackendError> {
    for reference in iter {
        let mut reference = reference.map_err(gix_error)?;
        let name = GitPath(reference.name().shorten().to_vec());
        let target = object_id(&mut reference)?;
        push(NamedRef {
            name,
            target,
            upstream: None,
            ahead: 0,
            behind: 0,
        });
    }
    Ok(())
}

fn object_id(reference: &mut gix::Reference<'_>) -> Result<String, GitBackendError> {
    if let Some(id) = reference.target().try_id() {
        return Ok(id.to_string());
    }
    Ok(reference.peel_to_id().map_err(gix_error)?.to_string())
}

pub(crate) fn tracking(
    repo: &gix::Repository,
    name: &gix::refs::FullNameRef,
) -> Result<(Option<String>, u32, u32), GitBackendError> {
    let Some(tracking) = repo.branch_remote_tracking_ref_name(name, gix::remote::Direction::Fetch)
    else {
        return Ok((None, 0, 0));
    };
    let tracking = tracking.map_err(gix_error)?;
    let upstream = String::from_utf8_lossy(tracking.shorten()).into_owned();
    let Ok(mut upstream_ref) = repo.find_reference(tracking.as_ref()) else {
        return Ok((Some(upstream), 0, 0));
    };
    let Ok(mut local_ref) = repo.find_reference(name) else {
        return Ok((Some(upstream), 0, 0));
    };
    let local_id = object_id(&mut local_ref)?;
    let upstream_id = object_id(&mut upstream_ref)?;
    let (ahead, behind) = ahead_behind(repo, &local_id, &upstream_id)?;
    Ok((Some(upstream), ahead, behind))
}

fn ahead_behind(
    repo: &gix::Repository,
    local: &str,
    upstream: &str,
) -> Result<(u32, u32), GitBackendError> {
    let local = gix::ObjectId::from_hex(local.as_bytes()).map_err(gix_error)?;
    let upstream = gix::ObjectId::from_hex(upstream.as_bytes()).map_err(gix_error)?;
    if local == upstream {
        return Ok((0, 0));
    }
    Ok((
        count_hidden(repo, local, upstream)?,
        count_hidden(repo, upstream, local)?,
    ))
}

fn count_hidden(
    repo: &gix::Repository,
    tip: gix::ObjectId,
    hidden: gix::ObjectId,
) -> Result<u32, GitBackendError> {
    let mut count = 0_u32;
    for commit in repo
        .rev_walk([tip])
        .with_hidden([hidden])
        .all()
        .map_err(gix_error)?
    {
        commit.map_err(gix_error)?;
        count = count.saturating_add(1);
    }
    Ok(count)
}

fn collect_remotes(
    repo: &gix::Repository,
    snapshot: &mut RefSnapshot,
) -> Result<(), GitBackendError> {
    for name in repo.remote_names() {
        let remote = repo
            .find_remote(gix::bstr::BStr::new(name.as_slice()))
            .map_err(gix_error)?;
        let fetch_url = remote
            .url(gix::remote::Direction::Fetch)
            .map_or_else(Vec::new, |url| url.to_string().into_bytes());
        snapshot.remotes.push(Remote {
            name: GitPath(name.to_vec()),
            fetch_url,
        });
    }
    Ok(())
}

pub(crate) fn git_path_is_safe(path: &GitPath) -> bool {
    !path.0.starts_with(b"/") && !path.0.split(|byte| *byte == b'/').any(|part| part == b"..")
}

pub(crate) fn worktree_path(
    repository: &WorktreeRepository,
    path: &GitPath,
) -> Result<PathBuf, GitBackendError> {
    if !git_path_is_safe(path) {
        return Err(GitBackendError::from_message("unsafe path"));
    }
    Ok(repository
        .worktree_root
        .join(std::ffi::OsString::from_vec(path.0.clone())))
}

pub(crate) fn gix_error(error: impl std::fmt::Display) -> GitBackendError {
    GitBackendError::from_message(redact_secrets(&error.to_string()))
}

fn redact_secrets(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut rest = input;
    while let Some(scheme_end) = rest.find("://") {
        output.push_str(&rest[..scheme_end + 3]);
        rest = &rest[scheme_end + 3..];
        let end = rest
            .find(|character: char| {
                character == '/'
                    || character.is_whitespace()
                    || character == '\''
                    || character == '"'
            })
            .unwrap_or(rest.len());
        let hostish = &rest[..end];
        if let Some(at) = hostish.rfind('@') {
            let userinfo = &hostish[..at];
            if userinfo.contains(':') {
                output.push_str("***:***");
                output.push_str(&hostish[at..]);
                rest = &rest[end..];
                continue;
            }
        }
        output.push_str(hostish);
        rest = &rest[end..];
    }
    output.push_str(rest);
    output
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        sync::atomic::{AtomicUsize, Ordering},
    };

    use app_core::{
        GitHistoryQuery, GitIndexMutate, GitNetwork, GitObjectQuery, GitRefQuery,
        RepositoryDiscoverer, open_repository,
    };
    use git_cli::GitExecutable;
    use git_domain::{
        CommitRequest, DiffLineKind, GitPath, HeadStatus, HistoryReference, HistoryRequest,
        StatusEntry, WorktreeStatus,
    };

    use super::GixGit;

    static NEXT_REPOSITORY: AtomicUsize = AtomicUsize::new(0);

    struct Repository {
        path: std::path::PathBuf,
        git: GitExecutable,
    }

    impl Repository {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "gitronimo-git-gix-{}-{}",
                std::process::id(),
                NEXT_REPOSITORY.fetch_add(1, Ordering::Relaxed)
            ));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).expect("temporary repository directory should exist");
            let git =
                GitExecutable::discover().expect("Git should be installed for integration tests");
            let repository = Self { path, git };
            repository.success(["init", "--initial-branch=main"]);
            repository.success(["config", "user.email", "test@gitronimo.invalid"]);
            repository.success(["config", "user.name", "GitRonimo Test"]);
            repository
        }

        fn success<I, S>(&self, args: I)
        where
            I: IntoIterator<Item = S>,
            S: AsRef<std::ffi::OsStr>,
        {
            let output = self.git.run(&self.path, args).expect("Git should run");
            assert!(output.status.success(), "Git command failed: {output:?}");
        }

        fn commit(&self, subject: &str) {
            fs::write(self.path.join("fixture.txt"), subject).expect("fixture file should write");
            self.success(["add", "fixture.txt"]);
            self.success(["commit", "-m", subject]);
        }
    }

    impl Drop for Repository {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn worktree_of(repository: &Repository) -> git_domain::WorktreeRepository {
        open_repository(&GixGit, &repository.path).expect("fixture should be a worktree")
    }

    #[test]
    fn discover_matches_git_cli_on_a_worktree() {
        let repository = Repository::new();
        repository.commit("initial");
        let gix = GixGit
            .discover_repository(&repository.path)
            .expect("gix should discover");
        let cli = repository
            .git
            .discover_repository(&repository.path)
            .expect("git should discover");
        assert_eq!(gix, cli);
        let nested = repository.path.join("nested");
        fs::create_dir(&nested).expect("nested directory should exist");
        assert_eq!(
            GixGit
                .discover_repository(&nested)
                .expect("nested discover"),
            repository
                .git
                .discover_repository(&nested)
                .expect("git nested discover")
        );
    }

    #[test]
    fn discover_rejects_a_plain_directory() {
        let path = std::env::temp_dir().join(format!(
            "gitronimo-git-gix-empty-{}-{}",
            std::process::id(),
            NEXT_REPOSITORY.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("empty directory should exist");
        assert!(matches!(
            GixGit.discover_repository(&path),
            Err(app_core::RepositoryOpenError::NotRepository(_))
        ));
        let _ = fs::remove_dir_all(&path);
    }

    #[test]
    fn head_status_matches_git_cli_for_branch_detached_and_unborn() {
        let repository = Repository::new();
        let unborn = worktree_of(&repository);
        assert_eq!(
            GixGit.head_status(&unborn).expect("gix unborn"),
            repository.git.head_status(&unborn).expect("git unborn")
        );

        repository.commit("initial");
        let worktree = worktree_of(&repository);
        assert_eq!(
            GixGit.head_status(&worktree).expect("gix branch"),
            repository.git.head_status(&worktree).expect("git branch")
        );
        assert_eq!(
            GixGit.head_oid(&worktree).expect("gix oid"),
            repository.git.head_oid(&worktree).expect("git oid")
        );

        let oid = repository.git.head_oid(&worktree).expect("oid");
        repository
            .git
            .checkout_detached(&worktree, &oid)
            .expect("detach");
        let detached = worktree_of(&repository);
        assert_eq!(
            GixGit.head_status(&detached).expect("gix detached"),
            HeadStatus::Detached
        );
        assert_eq!(
            GixGit.head_status(&detached).expect("gix detached"),
            repository.git.head_status(&detached).expect("git detached")
        );
    }

    #[test]
    fn ref_snapshot_matches_git_cli_names_and_tracking() {
        let repository = Repository::new();
        repository.commit("initial");
        repository.success(["branch", "feature/nested"]);
        repository.success(["tag", "v1.0.0"]);
        let remote = repository.path.with_extension("refs.git");
        repository.success([
            "clone",
            "--bare",
            ".",
            remote.to_str().expect("temporary path is UTF-8"),
        ]);
        repository.success([
            "remote",
            "add",
            "origin",
            remote.to_str().expect("temporary path is UTF-8"),
        ]);
        repository.success(["fetch", "origin"]);
        repository.success(["branch", "--set-upstream-to=origin/main", "main"]);
        let worktree = worktree_of(&repository);
        let gix = GixGit.ref_snapshot(&worktree).expect("gix snapshot");
        let cli = repository
            .git
            .ref_snapshot(&worktree)
            .expect("git snapshot");
        assert_eq!(
            sorted_names(&gix.local_branches),
            sorted_names(&cli.local_branches)
        );
        assert_eq!(
            sorted_names(&gix.remote_branches),
            sorted_names(&cli.remote_branches)
        );
        assert_eq!(sorted_names(&gix.tags), sorted_names(&cli.tags));
        assert_eq!(
            gix.remotes
                .iter()
                .map(|remote| remote.name.clone())
                .collect::<Vec<_>>(),
            cli.remotes
                .iter()
                .map(|remote| remote.name.clone())
                .collect::<Vec<_>>()
        );
        let gix_main = gix
            .local_branches
            .iter()
            .find(|branch| branch.name.0 == b"main")
            .expect("gix main");
        let cli_main = cli
            .local_branches
            .iter()
            .find(|branch| branch.name.0 == b"main")
            .expect("git main");
        assert_eq!(gix_main.target, cli_main.target);
        assert_eq!(gix_main.upstream, cli_main.upstream);
        assert_eq!(
            (gix_main.ahead, gix_main.behind),
            (cli_main.ahead, cli_main.behind)
        );
        let _ = fs::remove_dir_all(remote);
    }

    #[test]
    fn worktree_status_matches_git_cli_for_untracked_modified_staged_and_ignored() {
        let repository = Repository::new();
        repository.commit("initial");
        let filename = "sp ace\tand\nunicode-é.txt";
        fs::write(repository.path.join(filename), "untracked")
            .expect("unusual filename should write");
        fs::write(repository.path.join("fixture.txt"), "unstaged")
            .expect("modified file should write");
        let worktree = worktree_of(&repository);
        assert_status_matches(&repository.git, &worktree, false);

        repository.success(["add", "fixture.txt"]);
        assert_status_matches(&repository.git, &worktree, false);

        fs::write(repository.path.join("fixture.txt"), "both")
            .expect("second modification should write");
        assert_status_matches(&repository.git, &worktree, false);

        fs::write(repository.path.join(".gitignore"), "ignored.txt\n")
            .expect("ignore file should write");
        fs::write(repository.path.join("ignored.txt"), "ignored")
            .expect("ignored file should write");
        assert_status_matches(&repository.git, &worktree, false);
        assert_status_matches(&repository.git, &worktree, true);
        let with_ignored = GixGit
            .worktree_status(&worktree, true)
            .expect("gix ignored status");
        assert!(
            with_ignored
                .entries
                .contains(&StatusEntry::Ignored(GitPath(b"ignored.txt".to_vec())))
        );
        assert!(
            with_ignored
                .entries
                .contains(&StatusEntry::Untracked(GitPath(
                    filename.as_bytes().to_vec()
                )))
        );
    }

    #[test]
    fn worktree_status_matches_git_cli_stash_count_and_worktree_delete() {
        let repository = Repository::new();
        repository.commit("initial");
        fs::write(repository.path.join("fixture.txt"), "stash me")
            .expect("stash fixture should write");
        repository.success(["stash", "push", "-m", "fixture"]);
        let worktree = worktree_of(&repository);
        assert_status_matches(&repository.git, &worktree, false);
        let stashed = GixGit
            .worktree_status(&worktree, false)
            .expect("gix stash status");
        assert_eq!(stashed.stash_count, 1);

        fs::remove_file(repository.path.join("fixture.txt")).expect("tracked file should delete");
        assert_status_matches(&repository.git, &worktree, false);
        fs::write(repository.path.join("new.txt"), "added").expect("new file should write");
        repository.success(["add", "new.txt"]);
        assert_status_matches(&repository.git, &worktree, false);
    }

    #[test]
    fn history_page_matches_git_cli_for_current_and_pagination() {
        let repository = Repository::new();
        repository.commit("one");
        repository.commit("two");
        repository.commit("three");
        let worktree = worktree_of(&repository);
        let request = HistoryRequest {
            reference: HistoryReference::Current,
            before: None,
            limit: 2,
        };
        let gix = GixGit
            .history_page(&worktree, &request)
            .expect("gix history");
        let cli = repository
            .git
            .history_page(&worktree, &request)
            .expect("git history");
        assert_eq!(
            gix.commits
                .iter()
                .map(|commit| commit.oid.clone())
                .collect::<Vec<_>>(),
            cli.commits
                .iter()
                .map(|commit| commit.oid.clone())
                .collect::<Vec<_>>()
        );
        assert_eq!(gix.commits[0].subject, b"three");
        assert_eq!(gix.commits[0].author.name, cli.commits[0].author.name);
        let next = gix.next_before.clone().expect("page continues");
        assert_eq!(gix.next_before, cli.next_before);
        let page_two = GixGit
            .history_page(
                &worktree,
                &HistoryRequest {
                    reference: HistoryReference::Current,
                    before: Some(next),
                    limit: 2,
                },
            )
            .expect("gix page two");
        assert_eq!(page_two.commits[0].subject, b"one");
    }

    #[test]
    fn tree_and_blob_match_git_cli() {
        let repository = Repository::new();
        fs::create_dir(repository.path.join("dir")).expect("dir");
        fs::write(repository.path.join("dir/nested.txt"), "nested").expect("nested file");
        repository.success(["add", "dir/nested.txt"]);
        repository.success(["commit", "-m", "tree"]);
        let worktree = worktree_of(&repository);
        let oid = repository.git.head_oid(&worktree).expect("oid");
        let gix_root = GixGit
            .tree_entries(&worktree, &oid, &GitPath(Vec::new()))
            .expect("gix tree");
        let cli_root = repository
            .git
            .tree_entries(&worktree, &oid, &GitPath(Vec::new()))
            .expect("git tree");
        assert_eq!(gix_root, cli_root);
        let nested = GixGit
            .file_at_revision(&worktree, &oid, &GitPath(b"dir/nested.txt".to_vec()))
            .expect("gix blob");
        assert_eq!(nested, b"nested");
        assert_eq!(
            nested,
            repository
                .git
                .file_at_revision(&worktree, &oid, &GitPath(b"dir/nested.txt".to_vec()))
                .expect("git blob")
        );
    }

    #[test]
    fn file_diff_hunk_kinds_match_git_cli() {
        let repository = Repository::new();
        repository.commit("initial");
        fs::write(repository.path.join("fixture.txt"), "changed\n").expect("modify");
        let worktree = worktree_of(&repository);
        let path = GitPath(b"fixture.txt".to_vec());
        let gix = GixGit
            .file_diff_with_limit(&worktree, &path, false, 64_000)
            .expect("gix unstaged");
        let cli = repository
            .git
            .file_diff_with_limit(&worktree, &path, false, 64_000)
            .expect("git unstaged");
        assert_eq!(diff_kinds(&gix.diff), diff_kinds(&cli.diff));
        repository.success(["add", "fixture.txt"]);
        let gix_staged = GixGit
            .file_diff_with_limit(&worktree, &path, true, 64_000)
            .expect("gix staged");
        let cli_staged = repository
            .git
            .file_diff_with_limit(&worktree, &path, true, 64_000)
            .expect("git staged");
        assert_eq!(diff_kinds(&gix_staged.diff), diff_kinds(&cli_staged.diff));
    }

    #[test]
    fn stage_unstage_and_commit_round_trip() {
        let repository = Repository::new();
        repository.commit("initial");
        fs::write(repository.path.join("fixture.txt"), "staged\n").expect("modify");
        let worktree = worktree_of(&repository);
        let path = GitPath(b"fixture.txt".to_vec());
        GixGit
            .stage_paths(&worktree, std::slice::from_ref(&path))
            .expect("stage");
        let staged = GixGit
            .worktree_status(&worktree, false)
            .expect("status after stage");
        assert!(staged.entries.iter().any(|entry| matches!(
            entry,
            StatusEntry::Ordinary {
                path: staged_path,
                status: git_domain::FileStatus([b'M', b'.']),
                ..
            } if staged_path == &path
        )));
        GixGit
            .unstage_paths(&worktree, std::slice::from_ref(&path))
            .expect("unstage");
        GixGit
            .stage_paths(&worktree, std::slice::from_ref(&path))
            .expect("stage again");
        GixGit
            .commit(
                &worktree,
                &CommitRequest {
                    subject: "gix commit".into(),
                    body: String::new(),
                    amend: false,
                    sign_off: false,
                },
            )
            .expect("commit");
        let page = GixGit
            .history_page(
                &worktree,
                &HistoryRequest {
                    reference: HistoryReference::Current,
                    before: None,
                    limit: 1,
                },
            )
            .expect("history");
        assert_eq!(page.commits[0].subject, b"gix commit");
        GixGit
            .commit(
                &worktree,
                &CommitRequest {
                    subject: "amended".into(),
                    body: String::new(),
                    amend: true,
                    sign_off: true,
                },
            )
            .expect("amend");
        let amended = GixGit
            .history_page(
                &worktree,
                &HistoryRequest {
                    reference: HistoryReference::Current,
                    before: None,
                    limit: 1,
                },
            )
            .expect("amended history");
        assert_eq!(amended.commits[0].subject, b"amended");
        assert!(String::from_utf8_lossy(&amended.commits[0].body).contains("Signed-off-by:"));
    }

    #[test]
    fn http_url_routing_refuses_ssh_and_file() {
        assert!(crate::uses_http_url("https://example.com/repo.git"));
        assert!(crate::uses_http_url("http://example.com/repo.git"));
        assert!(!crate::uses_http_url("ssh://git@example.com/repo.git"));
        assert!(!crate::uses_http_url("git@example.com:repo.git"));
        assert!(!crate::uses_http_url("/tmp/repo.git"));
        let interrupt = std::sync::atomic::AtomicBool::new(false);
        let err = GixGit
            .clone_repository(
                "file:///tmp/not-a-repo",
                std::path::Path::new("/tmp/gitronimo-nope"),
                &interrupt,
            )
            .expect_err("file clone should refuse");
        assert!(err.message().contains("system Git"));
    }

    fn diff_kinds(diff: &git_domain::UnifiedDiff) -> Vec<(Option<Vec<u8>>, Vec<DiffLineKind>)> {
        diff.files
            .iter()
            .map(|file| {
                let path = file
                    .new_path
                    .as_ref()
                    .or(file.old_path.as_ref())
                    .map(|path| path.0.clone());
                let kinds = file
                    .hunks
                    .iter()
                    .flat_map(|hunk| hunk.lines.iter().map(|line| line.kind))
                    .collect();
                (path, kinds)
            })
            .collect()
    }

    fn assert_status_matches(
        git: &GitExecutable,
        worktree: &git_domain::WorktreeRepository,
        include_ignored: bool,
    ) {
        let gix = GixGit
            .worktree_status(worktree, include_ignored)
            .expect("gix status");
        let cli = git
            .worktree_status(worktree, include_ignored)
            .expect("git status");
        assert_eq!(gix.branch.head, cli.branch.head);
        assert_eq!(gix.branch.oid, cli.branch.oid);
        assert_eq!(gix.branch.upstream, cli.branch.upstream);
        assert_eq!(
            (gix.branch.ahead, gix.branch.behind),
            (cli.branch.ahead, cli.branch.behind)
        );
        assert_eq!(gix.stash_count, cli.stash_count);
        assert_eq!(gix.operation, cli.operation);
        assert_eq!(sorted_status_entries(&gix), sorted_status_entries(&cli));
    }

    fn sorted_status_entries(status: &WorktreeStatus) -> Vec<StatusEntry> {
        let mut entries = status.entries.clone();
        entries.sort_by(|left, right| status_entry_path(left).cmp(status_entry_path(right)));
        entries
    }

    fn status_entry_path(entry: &StatusEntry) -> &[u8] {
        match entry {
            StatusEntry::Ordinary { path, .. }
            | StatusEntry::Renamed { path, .. }
            | StatusEntry::Unmerged { path, .. }
            | StatusEntry::Untracked(path)
            | StatusEntry::Ignored(path) => &path.0,
        }
    }

    fn sorted_names(refs: &[git_domain::NamedRef]) -> Vec<Vec<u8>> {
        let mut names: Vec<Vec<u8>> = refs.iter().map(|entry| entry.name.0.clone()).collect();
        names.sort();
        names
    }
}
