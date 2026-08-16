//! Low-level index updates and commits via `gix`.

use std::os::unix::fs::PermissionsExt;

use crate::{git_path_is_safe, gix_error, worktree_path};
use app_core::GitBackendError;
use git_domain::{CommitRequest, GitPath, StatusEntry};
use gix::bstr::{BStr, BString};
use gix::index::entry::{Flags, Mode, Stage, Stat};
use gix::object::tree::EntryKind;

/// Stages paths from the worktree into the index, including deletions.
///
/// # Errors
/// Returns when a path is unsafe or the index cannot be written.
pub(crate) fn stage_paths(
    repo: &gix::Repository,
    repository: &git_domain::WorktreeRepository,
    paths: &[GitPath],
) -> Result<(), GitBackendError> {
    let mut index = open_index(repo);
    for path in paths {
        stage_one(repo, repository, &mut index, path)?;
    }
    write_index(&mut index)
}

/// Restores index entries from `HEAD`, or removes them when `HEAD` is unborn.
///
/// # Errors
/// Returns when a path is unsafe or the index cannot be written.
pub(crate) fn unstage_paths(
    repo: &gix::Repository,
    repository: &git_domain::WorktreeRepository,
    paths: &[GitPath],
) -> Result<(), GitBackendError> {
    let mut index = open_index(repo);
    let head_tree = repo.head_tree().ok();
    for path in paths {
        if !git_path_is_safe(path) {
            return Err(GitBackendError::from_message("unsafe path"));
        }
        match &head_tree {
            Some(tree) => restore_from_tree(repo, repository, &mut index, tree, path)?,
            None => remove_path(&mut index, path),
        }
    }
    write_index(&mut index)
}

/// Stages every worktree change, matching `git add -A`.
///
/// # Errors
/// Returns when status or the index cannot be written.
pub(crate) fn stage_all(
    repo: &gix::Repository,
    repository: &git_domain::WorktreeRepository,
) -> Result<(), GitBackendError> {
    let status = crate::status::worktree_status(repo, false)?;
    let paths: Vec<GitPath> = status
        .entries
        .into_iter()
        .filter_map(|entry| match entry {
            StatusEntry::Ignored(_) => None,
            StatusEntry::Ordinary { path, .. }
            | StatusEntry::Renamed { path, .. }
            | StatusEntry::Unmerged { path, .. }
            | StatusEntry::Untracked(path) => Some(path),
        })
        .collect();
    stage_paths(repo, repository, &paths)
}

/// Resets the index to `HEAD`, or empties it when `HEAD` is unborn.
///
/// # Errors
/// Returns when the index cannot be written.
pub(crate) fn unstage_all(repo: &gix::Repository) -> Result<(), GitBackendError> {
    let mut index = match repo.head_tree_id() {
        Ok(tree) => repo.index_from_tree(tree.as_ref()).map_err(gix_error)?,
        Err(_) => gix::index::File::from_state(
            gix::index::State::new(repo.object_hash()),
            repo.index_path(),
        ),
    };
    write_index(&mut index)
}

/// Writes a commit from the current index. Falls back to system Git for hooks and signing.
///
/// # Errors
/// Returns for empty subject, missing identity, empty non-amend commit, hooks, or `commit.gpgsign`.
pub(crate) fn commit(
    repo: &gix::Repository,
    request: &CommitRequest,
) -> Result<(), GitBackendError> {
    if request.subject.trim().is_empty() {
        return Err(GitBackendError::from_message("invalid commit message"));
    }
    refuse_signed_or_hooked(repo)?;
    let author = repo
        .author()
        .ok_or_else(|| GitBackendError::from_message("missing identity"))?
        .map_err(gix_error)?;
    let index = open_index(repo);
    let tree = tree_id_from_index(repo, &index)?;
    if !request.amend {
        refuse_empty_commit(repo, tree)?;
    }
    let message = commit_message(request, &author);
    if request.amend {
        let head = repo.head_commit().map_err(gix_error)?;
        let parents: Vec<gix::ObjectId> = head.parent_ids().map(gix::Id::detach).collect();
        let commit = repo
            .new_commit(&message, tree, parents)
            .map_err(gix_error)?;
        let head = repo.head().map_err(gix_error)?;
        let log = format!("commit (amend): {}", request.subject.trim());
        if let Some(name) = head.referent_name() {
            let mut branch = repo.find_reference(name).map_err(gix_error)?;
            branch.set_target_id(commit.id, log).map_err(gix_error)?;
        } else {
            let mut head_ref = repo.find_reference("HEAD").map_err(gix_error)?;
            head_ref.set_target_id(commit.id, log).map_err(gix_error)?;
        }
        return Ok(());
    }
    let parents: Vec<gix::ObjectId> = match repo.head_id() {
        Ok(id) => vec![id.detach()],
        Err(_) => Vec::new(),
    };
    if parents.is_empty() {
        let head = repo.head().map_err(gix_error)?;
        let name = head
            .referent_name()
            .ok_or_else(|| GitBackendError::from_message("unborn HEAD has no branch"))?;
        repo.commit(name.to_owned(), &message, tree, parents)
            .map_err(gix_error)?;
    } else {
        repo.commit("HEAD", &message, tree, parents)
            .map_err(gix_error)?;
    }
    Ok(())
}

/// Builds a tree object from every unconflicted index entry.
///
/// # Errors
/// Returns when tree editing or object writes fail.
pub(crate) fn tree_id_from_index(
    repo: &gix::Repository,
    index: &gix::index::File,
) -> Result<gix::ObjectId, GitBackendError> {
    let mut editor = repo.empty_tree().edit().map_err(gix_error)?;
    for entry in index.entries() {
        if entry.stage() != Stage::Unconflicted {
            continue;
        }
        let Some(mode) = entry.mode.to_tree_entry_mode() else {
            continue;
        };
        if mode.kind() == EntryKind::Tree {
            continue;
        }
        editor
            .upsert(
                BString::from(entry.path(index).to_vec()),
                mode.kind(),
                entry.id,
            )
            .map_err(gix_error)?;
    }
    Ok(editor.write().map_err(gix_error)?.detach())
}

fn refuse_empty_commit(repo: &gix::Repository, tree: gix::ObjectId) -> Result<(), GitBackendError> {
    match repo.head_commit() {
        Ok(commit) => {
            let parent_tree = commit.tree_id().map_err(gix_error)?;
            if parent_tree.detach() == tree {
                return Err(GitBackendError::from_message("nothing to commit"));
            }
        }
        Err(_) => {
            if tree == repo.empty_tree().id {
                return Err(GitBackendError::from_message("nothing to commit"));
            }
        }
    }
    Ok(())
}

fn refuse_signed_or_hooked(repo: &gix::Repository) -> Result<(), GitBackendError> {
    if repo.config_snapshot().boolean("commit.gpgsign") == Some(true) {
        return Err(GitBackendError::from_message(
            "signed commits require system Git",
        ));
    }
    let hooks = repo.common_dir().join("hooks");
    for name in [
        "pre-commit",
        "prepare-commit-msg",
        "commit-msg",
        "post-commit",
    ] {
        let path = hooks.join(name);
        if is_executable_hook(&path) {
            return Err(GitBackendError::from_message(
                "commit hooks require system Git",
            ));
        }
    }
    Ok(())
}

fn is_executable_hook(path: &std::path::Path) -> bool {
    let Ok(meta) = path.metadata() else {
        return false;
    };
    meta.is_file() && meta.permissions().mode() & 0o111 != 0
}

fn commit_message(request: &CommitRequest, author: &gix::actor::SignatureRef<'_>) -> String {
    let mut message = request.subject.trim().to_owned();
    if !request.body.trim().is_empty() {
        message.push_str("\n\n");
        message.push_str(request.body.trim_end());
    }
    if request.sign_off {
        let name = String::from_utf8_lossy(author.name);
        let email = String::from_utf8_lossy(author.email);
        message.push_str("\n\nSigned-off-by: ");
        message.push_str(&name);
        message.push_str(" <");
        message.push_str(&email);
        message.push('>');
    }
    message
}

fn stage_one(
    repo: &gix::Repository,
    repository: &git_domain::WorktreeRepository,
    index: &mut gix::index::File,
    path: &GitPath,
) -> Result<(), GitBackendError> {
    if !git_path_is_safe(path) {
        return Err(GitBackendError::from_message("unsafe path"));
    }
    let full = worktree_path(repository, path)?;
    if !full.exists() {
        remove_path(index, path);
        return Ok(());
    }
    let meta = gix::index::fs::Metadata::from_path_no_follow(&full).map_err(gix_error)?;
    let (mode, bytes) = if meta.is_symlink() {
        let target = std::fs::read_link(&full).map_err(gix_error)?;
        (
            Mode::SYMLINK,
            std::os::unix::ffi::OsStringExt::into_vec(target.into_os_string()),
        )
    } else if meta.is_executable() {
        (
            Mode::FILE_EXECUTABLE,
            std::fs::read(&full).map_err(gix_error)?,
        )
    } else {
        (Mode::FILE, std::fs::read(&full).map_err(gix_error)?)
    };
    let oid = repo.write_blob(&bytes).map_err(gix_error)?.detach();
    let stat = Stat::from_fs(&meta).map_err(gix_error)?;
    upsert_entry(index, path, stat, oid, mode);
    Ok(())
}

fn restore_from_tree(
    repo: &gix::Repository,
    repository: &git_domain::WorktreeRepository,
    index: &mut gix::index::File,
    tree: &gix::Tree<'_>,
    path: &GitPath,
) -> Result<(), GitBackendError> {
    let entry = tree
        .lookup_entry(
            path.0
                .split(|byte| *byte == b'/')
                .filter(|component| !component.is_empty())
                .map(gix::bstr::BString::from),
        )
        .map_err(gix_error)?;
    let Some(entry) = entry else {
        remove_path(index, path);
        return Ok(());
    };
    let Some(mode) = Mode::from_bits(u32::from(entry.mode().value())) else {
        remove_path(index, path);
        return Ok(());
    };
    let stat = match worktree_path(repository, path) {
        Ok(full) => gix::index::fs::Metadata::from_path_no_follow(&full)
            .ok()
            .and_then(|meta| Stat::from_fs(&meta).ok())
            .unwrap_or_default(),
        Err(_) => Stat::default(),
    };
    let _ = repo;
    upsert_entry(index, path, stat, entry.object_id(), mode);
    Ok(())
}

fn upsert_entry(
    index: &mut gix::index::File,
    path: &GitPath,
    stat: Stat,
    id: gix::ObjectId,
    mode: Mode,
) {
    remove_path(index, path);
    index.dangerously_push_entry(stat, id, Flags::empty(), mode, BStr::new(&path.0));
    index.sort_entries();
}

fn remove_path(index: &mut gix::index::File, path: &GitPath) {
    let path_bstr = BStr::new(&path.0);
    let mut index_pos = 0;
    while index_pos < index.entries().len() {
        if index.entries()[index_pos].path(index) == path_bstr {
            index.remove_entry_at_index(index_pos);
        } else {
            index_pos = index_pos.saturating_add(1);
        }
    }
}

fn open_index(repo: &gix::Repository) -> gix::index::File {
    match repo.open_index() {
        Ok(index) => index,
        Err(_) => gix::index::File::from_state(
            gix::index::State::new(repo.object_hash()),
            repo.index_path(),
        ),
    }
}

fn write_index(index: &mut gix::index::File) -> Result<(), GitBackendError> {
    index.remove_tree();
    index
        .write(gix::index::write::Options::default())
        .map_err(gix_error)
}
