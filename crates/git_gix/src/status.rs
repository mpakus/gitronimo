//! Index / worktree status mapped onto the porcelain-v2 `git_domain` model.
//!
//! gitui (MIT) `asyncgit/src/sync/status.rs` uses `gix::status` `into_iter()`;
//! this mapper is original and keeps XY codes the Working Copy expects.

use std::collections::{HashMap, HashSet};

use app_core::GitBackendError;
use git_domain::{
    BranchStatus, FileStatus, GitPath, InProgressOperation, RenameKind, StatusEntry,
    SubmoduleState, WorktreeStatus,
};
use gix::diff::index::ChangeRef;
use gix::dir::entry::Status as DirStatus;
use gix::status::Item;
use gix::status::index_worktree::Item as IndexWorktreeItem;
use gix::status::plumbing::index_as_worktree::{Change, Conflict, EntryStatus};

use crate::{gix_error, head_status, tracking};

/// Builds [`WorktreeStatus`] from `gix` without writing index stat updates.
///
/// # Errors
/// Returns when status, HEAD, tracking, or the stash reflog cannot be read.
pub(crate) fn worktree_status(
    repo: &gix::Repository,
    include_ignored: bool,
) -> Result<WorktreeStatus, GitBackendError> {
    let mut status = WorktreeStatus {
        branch: branch_status(repo)?,
        stash_count: stash_count(repo)?,
        operation: in_progress_operation(repo),
        entries: Vec::new(),
    };
    status.entries = collect_entries(repo, include_ignored)?;
    Ok(status)
}

fn branch_status(repo: &gix::Repository) -> Result<BranchStatus, GitBackendError> {
    let head = head_status(repo)?;
    let oid = repo.head_id().ok().map(|id| id.to_string().into_bytes());
    let (upstream, ahead, behind) = match repo.head() {
        Ok(git_head) => match git_head.referent_name() {
            Some(name) => {
                let (upstream, ahead, behind) = tracking(repo, name)?;
                (
                    upstream.map(|name| GitPath(name.into_bytes())),
                    ahead,
                    behind,
                )
            }
            None => (None, 0, 0),
        },
        Err(_) => (None, 0, 0),
    };
    Ok(BranchStatus {
        oid,
        head,
        upstream,
        ahead,
        behind,
    })
}

fn stash_count(repo: &gix::Repository) -> Result<u32, GitBackendError> {
    let Ok(reference) = repo.find_reference("refs/stash") else {
        return Ok(0);
    };
    let mut log = reference.log_iter();
    match log.all().map_err(gix_error)? {
        None => Ok(1),
        Some(entries) => u32::try_from(entries.filter_map(Result::ok).count())
            .map_err(|_| GitBackendError::from_message("too many stashes")),
    }
}

fn in_progress_operation(repo: &gix::Repository) -> InProgressOperation {
    let git_dir = repo.git_dir();
    let merge_head = git_dir.join("MERGE_HEAD");
    if merge_head.exists() {
        return InProgressOperation::Merge {
            oid: read_state_oid(&merge_head),
        };
    }
    if git_dir.join("rebase-merge").is_dir() || git_dir.join("rebase-apply").is_dir() {
        return InProgressOperation::Rebase;
    }
    let cherry_pick_head = git_dir.join("CHERRY_PICK_HEAD");
    if cherry_pick_head.exists() {
        return InProgressOperation::CherryPick {
            oid: read_state_oid(&cherry_pick_head),
        };
    }
    let revert_head = git_dir.join("REVERT_HEAD");
    if revert_head.exists() {
        return InProgressOperation::Revert {
            oid: read_state_oid(&revert_head),
        };
    }
    InProgressOperation::None
}

fn read_state_oid(path: &std::path::Path) -> Option<Vec<u8>> {
    let oid = std::fs::read(path).ok()?;
    let oid = oid
        .into_iter()
        .filter(|byte| !byte.is_ascii_whitespace())
        .collect::<Vec<_>>();
    (!oid.is_empty()).then_some(oid)
}

fn collect_entries(
    repo: &gix::Repository,
    include_ignored: bool,
) -> Result<Vec<StatusEntry>, GitBackendError> {
    let mut platform = repo.status(gix::progress::Discard).map_err(gix_error)?;
    if include_ignored {
        platform = platform.dirwalk_options(|options| {
            options.emit_ignored(Some(gix::dir::walk::EmissionMode::Matching))
        });
    }
    let iter = platform
        .index_worktree_rewrites(None)
        .into_iter(Vec::new())
        .map_err(gix_error)?;
    let mut items = Vec::new();
    for item in iter {
        items.push(item.map_err(gix_error)?);
    }
    Ok(entries_from_items(items))
}

#[derive(Clone)]
struct Ordinary {
    index: u8,
    worktree: u8,
    submodule: SubmoduleState,
}

impl Default for Ordinary {
    fn default() -> Self {
        Self {
            index: b'.',
            worktree: b'.',
            submodule: SubmoduleState::NotSubmodule,
        }
    }
}

fn entries_from_items(items: Vec<Item>) -> Vec<StatusEntry> {
    let rename_sources = rename_sources(&items);
    let mut ordinary = HashMap::<Vec<u8>, Ordinary>::new();
    let mut renamed = Vec::new();
    let mut unmerged = Vec::new();
    let mut untracked = Vec::new();
    let mut ignored = Vec::new();

    for item in items {
        match item {
            Item::TreeIndex(change) => {
                apply_tree_index(change, &rename_sources, &mut ordinary, &mut renamed);
            }
            Item::IndexWorktree(change) => apply_index_worktree(
                change,
                &mut ordinary,
                &mut unmerged,
                &mut untracked,
                &mut ignored,
            ),
        }
    }

    overlay_renames(&mut ordinary, &mut renamed);
    assemble_entries(ordinary, renamed, unmerged, untracked, ignored)
}

fn rename_sources(items: &[Item]) -> HashSet<Vec<u8>> {
    items
        .iter()
        .filter_map(|item| {
            let Item::TreeIndex(ChangeRef::Rewrite {
                source_location,
                copy,
                ..
            }) = item
            else {
                return None;
            };
            (!*copy).then(|| source_location.as_ref().to_vec())
        })
        .collect()
}

fn apply_tree_index(
    change: gix::diff::index::Change,
    rename_sources: &HashSet<Vec<u8>>,
    ordinary: &mut HashMap<Vec<u8>, Ordinary>,
    renamed: &mut Vec<StatusEntry>,
) {
    match change {
        ChangeRef::Addition { location, .. } => {
            set_index(ordinary, location.as_ref().to_vec(), b'A');
        }
        ChangeRef::Deletion { location, .. } => {
            let path = location.as_ref().to_vec();
            if !rename_sources.contains(&path) {
                set_index(ordinary, path, b'D');
            }
        }
        ChangeRef::Modification { location, .. } => {
            set_index(ordinary, location.as_ref().to_vec(), b'M');
        }
        ChangeRef::Rewrite {
            source_location,
            location,
            copy,
            ..
        } => {
            let path = GitPath(location.as_ref().to_vec());
            let source_path = GitPath(source_location.as_ref().to_vec());
            renamed.push(StatusEntry::Renamed {
                status: FileStatus([if copy { b'C' } else { b'R' }, b'.']),
                submodule: SubmoduleState::NotSubmodule,
                kind: if copy {
                    RenameKind::Copy
                } else {
                    RenameKind::Rename
                },
                score: 100,
                path,
                source_path,
            });
        }
    }
}

fn apply_index_worktree(
    change: IndexWorktreeItem,
    ordinary: &mut HashMap<Vec<u8>, Ordinary>,
    unmerged: &mut Vec<StatusEntry>,
    untracked: &mut Vec<GitPath>,
    ignored: &mut Vec<GitPath>,
) {
    match change {
        IndexWorktreeItem::Modification {
            rela_path, status, ..
        } => apply_worktree_status(rela_path.to_vec(), status, ordinary, unmerged),
        IndexWorktreeItem::DirectoryContents { entry, .. } => match entry.status {
            DirStatus::Untracked => untracked.push(GitPath(entry.rela_path.to_vec())),
            DirStatus::Ignored(_) => ignored.push(GitPath(entry.rela_path.to_vec())),
            DirStatus::Pruned | DirStatus::Tracked => {}
        },
        IndexWorktreeItem::Rewrite { dirwalk_entry, .. } => {
            untracked.push(GitPath(dirwalk_entry.rela_path.to_vec()));
        }
    }
}

fn apply_worktree_status(
    path: Vec<u8>,
    status: EntryStatus<(), gix::submodule::Status>,
    ordinary: &mut HashMap<Vec<u8>, Ordinary>,
    unmerged: &mut Vec<StatusEntry>,
) {
    match status {
        EntryStatus::Conflict { summary, .. } => {
            unmerged.push(StatusEntry::Unmerged {
                status: conflict_status(summary),
                submodule: SubmoduleState::NotSubmodule,
                path: GitPath(path),
            });
        }
        EntryStatus::Change(Change::Removed) => set_worktree(ordinary, path, b'D', None),
        EntryStatus::Change(Change::Type { .. }) => set_worktree(ordinary, path, b'T', None),
        EntryStatus::Change(Change::Modification { .. }) => {
            set_worktree(ordinary, path, b'M', None);
        }
        EntryStatus::Change(Change::SubmoduleModification(status)) => {
            set_worktree(ordinary, path, b'M', Some(submodule_state(&status)));
        }
        EntryStatus::IntentToAdd => set_index(ordinary, path, b'A'),
        EntryStatus::NeedsUpdate(_) => {}
    }
}

fn conflict_status(summary: Conflict) -> FileStatus {
    FileStatus(match summary {
        Conflict::BothDeleted => *b"DD",
        Conflict::AddedByUs => *b"AU",
        Conflict::DeletedByThem => *b"UD",
        Conflict::AddedByThem => *b"UA",
        Conflict::DeletedByUs => *b"DU",
        Conflict::BothAdded => *b"AA",
        Conflict::BothModified => *b"UU",
    })
}

fn submodule_state(status: &gix::submodule::Status) -> SubmoduleState {
    SubmoduleState::Changed {
        commit: status
            .checked_out_head_id
            .zip(status.index_id)
            .is_some_and(|(head, index)| head != index),
        modified: status
            .changes
            .as_ref()
            .is_some_and(|changes| !changes.is_empty()),
        untracked: false,
    }
}

fn set_index(ordinary: &mut HashMap<Vec<u8>, Ordinary>, path: Vec<u8>, letter: u8) {
    ordinary.entry(path).or_default().index = letter;
}

fn set_worktree(
    ordinary: &mut HashMap<Vec<u8>, Ordinary>,
    path: Vec<u8>,
    letter: u8,
    submodule: Option<SubmoduleState>,
) {
    let entry = ordinary.entry(path).or_default();
    entry.worktree = letter;
    if let Some(submodule) = submodule {
        entry.submodule = submodule;
    }
}

fn overlay_renames(ordinary: &mut HashMap<Vec<u8>, Ordinary>, renamed: &mut [StatusEntry]) {
    for entry in renamed.iter_mut() {
        let StatusEntry::Renamed {
            status,
            path,
            source_path,
            ..
        } = entry
        else {
            continue;
        };
        ordinary.remove(&source_path.0);
        if let Some(worktree) = ordinary.remove(&path.0) {
            status.0[1] = worktree.worktree;
        }
    }
}

fn assemble_entries(
    ordinary: HashMap<Vec<u8>, Ordinary>,
    renamed: Vec<StatusEntry>,
    unmerged: Vec<StatusEntry>,
    untracked: Vec<GitPath>,
    ignored: Vec<GitPath>,
) -> Vec<StatusEntry> {
    let mut entries = Vec::with_capacity(
        ordinary.len() + renamed.len() + unmerged.len() + untracked.len() + ignored.len(),
    );
    entries.extend(unmerged);
    entries.extend(ordinary.into_iter().filter_map(|(path, ordinary)| {
        (ordinary.index != b'.' || ordinary.worktree != b'.').then_some(StatusEntry::Ordinary {
            status: FileStatus([ordinary.index, ordinary.worktree]),
            submodule: ordinary.submodule,
            path: GitPath(path),
        })
    }));
    entries.extend(renamed);
    entries.extend(untracked.into_iter().map(StatusEntry::Untracked));
    entries.extend(ignored.into_iter().map(StatusEntry::Ignored));
    entries.sort_by(|left, right| entry_sort_key(left).cmp(entry_sort_key(right)));
    entries
}

fn entry_sort_key(entry: &StatusEntry) -> &[u8] {
    match entry {
        StatusEntry::Ordinary { path, .. }
        | StatusEntry::Renamed { path, .. }
        | StatusEntry::Unmerged { path, .. }
        | StatusEntry::Untracked(path)
        | StatusEntry::Ignored(path) => &path.0,
    }
}
