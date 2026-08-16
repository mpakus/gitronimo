//! Tree, blob, and unified-diff reads via `gix`.

use std::collections::HashMap;

use app_core::GitBackendError;
use git_domain::{
    DiffFile, DiffHunk, DiffLine, DiffLineKind, GitPath, LoadedDiff, MAX_DISPLAY_DIFF_BYTES,
    TreeEntry, TreeEntryKind, UnifiedDiff,
};
use gix::bstr::{BStr, ByteSlice};
use gix::diff::blob::{
    Algorithm, InternedInput, UnifiedDiff as GixUnifiedDiff, diff_with_slider_heuristics,
    unified_diff::{ConsumeHunk, ContextSize, DiffLineKind as GixLineKind, HunkHeader},
};
use gix::object::tree::{EntryKind, EntryMode};

use crate::{git_path_is_safe, gix_error, worktree_path};

/// Lists one tree directory. An empty path lists the commit's root tree.
///
/// # Errors
/// Returns when the revision cannot be peeled to a tree.
pub(crate) fn tree_entries(
    repo: &gix::Repository,
    oid: &str,
    path: &GitPath,
) -> Result<Vec<TreeEntry>, GitBackendError> {
    let tree = tree_at(repo, oid, path)?;
    let mut entries = Vec::new();
    for entry in tree.iter() {
        let entry = entry.map_err(gix_error)?;
        let kind = match entry.kind() {
            EntryKind::Tree => TreeEntryKind::Tree,
            EntryKind::Commit => TreeEntryKind::Commit,
            EntryKind::Blob | EntryKind::BlobExecutable | EntryKind::Link => TreeEntryKind::Blob,
        };
        entries.push(TreeEntry {
            name: GitPath(entry.filename().to_vec()),
            kind,
            oid: entry.id().to_string().into_bytes(),
            mode: ls_tree_mode(entry.kind()),
        });
    }
    Ok(entries)
}

/// Reads a blob at `oid:path`.
///
/// # Errors
/// Returns when the path is not a blob.
pub(crate) fn file_at_revision(
    repo: &gix::Repository,
    oid: &str,
    path: &GitPath,
) -> Result<Vec<u8>, GitBackendError> {
    if !git_path_is_safe(path) {
        return Err(GitBackendError::from_message("unsafe path"));
    }
    let tree = peel_to_tree(repo, oid)?;
    let entry = lookup_path(&tree, path)?
        .ok_or_else(|| GitBackendError::from_message("path not in tree"))?;
    blob_bytes(repo, entry.object_id())
}

/// Staged (`HEAD` vs index) or unstaged (index vs worktree) file diff.
///
/// # Errors
/// Returns when the index, HEAD, or worktree cannot be read.
pub(crate) fn file_diff_with_limit(
    repo: &gix::Repository,
    repository: &git_domain::WorktreeRepository,
    path: &GitPath,
    staged: bool,
    limit: usize,
) -> Result<LoadedDiff, GitBackendError> {
    if !git_path_is_safe(path) {
        return Err(GitBackendError::from_message("unsafe path"));
    }
    let (old, new) = if staged {
        (head_blob(repo, path)?, index_blob(repo, path)?)
    } else {
        (index_blob(repo, path)?, worktree_bytes(repository, path)?)
    };
    Ok(single_path_diff(
        path,
        old.as_deref(),
        new.as_deref(),
        limit,
    ))
}

/// Unified diff of a commit against its first parent (empty tree if root).
///
/// # Errors
/// Returns when the commit or its trees cannot be read.
pub(crate) fn commit_diff(
    repo: &gix::Repository,
    oid: &str,
    limit: usize,
) -> Result<LoadedDiff, GitBackendError> {
    let commit = peel_to_commit(repo, oid)?;
    let new_tree = commit.tree().map_err(gix_error)?;
    let old_tree = match commit.parent_ids().next() {
        Some(parent) => Some(
            repo.find_commit(parent.detach())
                .map_err(gix_error)?
                .tree()
                .map_err(gix_error)?,
        ),
        None => None,
    };
    trees_to_diff(repo, old_tree.as_ref(), Some(&new_tree), limit)
}

/// Two-dot unified diff between two refs.
///
/// # Errors
/// Returns when either ref cannot be peeled to a tree.
pub(crate) fn diff_refs(
    repo: &gix::Repository,
    left: &str,
    right: &str,
    limit: usize,
) -> Result<LoadedDiff, GitBackendError> {
    let old_tree = peel_to_tree(repo, left)?;
    let new_tree = peel_to_tree(repo, right)?;
    trees_to_diff(repo, Some(&old_tree), Some(&new_tree), limit)
}

/// Addition/deletion counts from staged and unstaged content diffs.
///
/// # Errors
/// Returns when HEAD, the index, or the worktree cannot be read.
pub(crate) fn diff_numstat(
    repo: &gix::Repository,
    repository: &git_domain::WorktreeRepository,
) -> Result<HashMap<GitPath, (u64, u64)>, GitBackendError> {
    let mut stats = HashMap::new();
    let index = open_index(repo);
    let head_tree = repo.head_tree().ok();
    let index_tree_id = crate::mutate::tree_id_from_index(repo, &index)?;
    let index_tree = repo.find_tree(index_tree_id).map_err(gix_error)?;
    accumulate_tree_numstat(repo, head_tree.as_ref(), Some(&index_tree), &mut stats)?;
    for entry in index.entries() {
        if entry.stage() != gix::index::entry::Stage::Unconflicted {
            continue;
        }
        let path = GitPath(entry.path(&index).to_vec());
        if !git_path_is_safe(&path) {
            continue;
        }
        let old = blob_bytes(repo, entry.id).ok();
        let new = worktree_bytes(repository, &path)?;
        add_counts(&mut stats, &path, old.as_deref(), new.as_deref());
    }
    let snapshot = crate::status::worktree_status(repo, false)?;
    for entry in snapshot.entries {
        let git_domain::StatusEntry::Untracked(path) = entry else {
            continue;
        };
        let new = worktree_bytes(repository, &path)?;
        add_counts(&mut stats, &path, None, new.as_deref());
    }
    Ok(stats)
}

fn accumulate_tree_numstat(
    repo: &gix::Repository,
    old: Option<&gix::Tree<'_>>,
    new: Option<&gix::Tree<'_>>,
    stats: &mut HashMap<GitPath, (u64, u64)>,
) -> Result<(), GitBackendError> {
    let loaded = trees_to_diff(repo, old, new, MAX_DISPLAY_DIFF_BYTES)?;
    for file in loaded.diff.files {
        let path = file
            .new_path
            .clone()
            .or(file.old_path)
            .unwrap_or_else(|| GitPath(Vec::new()));
        let mut added = 0_u64;
        let mut deleted = 0_u64;
        for hunk in file.hunks {
            for line in hunk.lines {
                match line.kind {
                    DiffLineKind::Addition => added = added.saturating_add(1),
                    DiffLineKind::Removal => deleted = deleted.saturating_add(1),
                    DiffLineKind::Context => {}
                }
            }
        }
        if added > 0 || deleted > 0 {
            let entry = stats.entry(path).or_insert((0, 0));
            entry.0 = entry.0.saturating_add(added);
            entry.1 = entry.1.saturating_add(deleted);
        }
    }
    Ok(())
}

fn trees_to_diff(
    repo: &gix::Repository,
    old: Option<&gix::Tree<'_>>,
    new: Option<&gix::Tree<'_>>,
    limit: usize,
) -> Result<LoadedDiff, GitBackendError> {
    let changes = repo.diff_tree_to_tree(old, new, None).map_err(gix_error)?;
    let mut files = Vec::new();
    let mut used = 0_usize;
    let mut truncated = false;
    for change in changes {
        if truncated {
            break;
        }
        let Some(file) = change_to_file(repo, change, limit, &mut used, &mut truncated)? else {
            continue;
        };
        files.push(file);
    }
    Ok(LoadedDiff {
        diff: UnifiedDiff { files },
        truncated,
    })
}

fn change_to_file(
    repo: &gix::Repository,
    change: gix::object::tree::diff::ChangeDetached,
    limit: usize,
    used: &mut usize,
    truncated: &mut bool,
) -> Result<Option<DiffFile>, GitBackendError> {
    use gix::object::tree::diff::ChangeDetached::{Addition, Deletion, Modification, Rewrite};
    let (old_path, new_path, old_id, new_id, mode) = match change {
        Addition {
            location,
            entry_mode,
            id,
            ..
        } => {
            if entry_mode.kind() == EntryKind::Tree {
                return Ok(None);
            }
            (
                None,
                Some(GitPath(location.to_vec())),
                None,
                Some(id),
                entry_mode,
            )
        }
        Deletion {
            location,
            entry_mode,
            id,
            ..
        } => {
            if entry_mode.kind() == EntryKind::Tree {
                return Ok(None);
            }
            (
                Some(GitPath(location.to_vec())),
                None,
                Some(id),
                None,
                entry_mode,
            )
        }
        Modification {
            location,
            previous_id,
            id,
            entry_mode,
            ..
        } => {
            if entry_mode.kind() == EntryKind::Tree {
                return Ok(None);
            }
            let path = GitPath(location.to_vec());
            (
                Some(path.clone()),
                Some(path),
                Some(previous_id),
                Some(id),
                entry_mode,
            )
        }
        Rewrite {
            source_location,
            location,
            source_id,
            id,
            entry_mode,
            ..
        } => {
            if entry_mode.kind() == EntryKind::Tree {
                return Ok(None);
            }
            (
                Some(GitPath(source_location.to_vec())),
                Some(GitPath(location.to_vec())),
                Some(source_id),
                Some(id),
                entry_mode,
            )
        }
    };
    let old = old_id
        .map(|id| blob_bytes(repo, id))
        .transpose()?
        .unwrap_or_default();
    let new = new_id
        .map(|id| blob_bytes(repo, id))
        .transpose()?
        .unwrap_or_default();
    *used = used.saturating_add(old.len().saturating_add(new.len()));
    if *used > limit {
        *truncated = true;
    }
    let path = new_path
        .clone()
        .or_else(|| old_path.clone())
        .unwrap_or_else(|| GitPath(Vec::new()));
    Ok(Some(blob_file_diff(
        old_path.as_ref().unwrap_or(&path),
        new_path.as_ref().unwrap_or(&path),
        &old,
        &new,
        is_binary_mode(mode),
        *truncated,
    )))
}

fn single_path_diff(
    path: &GitPath,
    old: Option<&[u8]>,
    new: Option<&[u8]>,
    limit: usize,
) -> LoadedDiff {
    let old = old.unwrap_or(b"");
    let new = new.unwrap_or(b"");
    let truncated = old.len().saturating_add(new.len()) > limit;
    LoadedDiff {
        diff: UnifiedDiff {
            files: vec![blob_file_diff(path, path, old, new, false, truncated)],
        },
        truncated,
    }
}

fn blob_file_diff(
    old_path: &GitPath,
    new_path: &GitPath,
    old: &[u8],
    new: &[u8],
    force_binary: bool,
    truncated: bool,
) -> DiffFile {
    let binary = force_binary || is_binary(old) || is_binary(new);
    if binary || truncated && old.len().saturating_add(new.len()) > MAX_DISPLAY_DIFF_BYTES {
        return DiffFile {
            old_path: empty_to_none(old, old_path),
            new_path: empty_to_none(new, new_path),
            binary: true,
            hunks: Vec::new(),
        };
    }
    DiffFile {
        old_path: empty_to_none(old, old_path),
        new_path: empty_to_none(new, new_path),
        binary: false,
        hunks: unified_hunks(old, new),
    }
}

fn empty_to_none(bytes: &[u8], path: &GitPath) -> Option<GitPath> {
    if bytes.is_empty() {
        None
    } else {
        Some(path.clone())
    }
}

fn unified_hunks(old: &[u8], new: &[u8]) -> Vec<DiffHunk> {
    if old == new {
        return Vec::new();
    }
    let input = InternedInput::new(old, new);
    let diff = diff_with_slider_heuristics(Algorithm::Histogram, &input);
    GixUnifiedDiff::new(
        &diff,
        &input,
        HunkCollector::default(),
        ContextSize::symmetrical(3),
    )
    .consume()
    .unwrap_or_default()
}

#[derive(Default)]
struct HunkCollector {
    hunks: Vec<DiffHunk>,
}

impl ConsumeHunk for HunkCollector {
    type Out = Vec<DiffHunk>;

    fn consume_hunk(
        &mut self,
        header: HunkHeader,
        lines: &[(GixLineKind, &[u8])],
    ) -> std::io::Result<()> {
        let mut old_line = u64::from(header.before_hunk_start);
        let mut new_line = u64::from(header.after_hunk_start);
        let mut diff_lines = Vec::with_capacity(lines.len());
        for (kind, content) in lines {
            let missing_final_newline = !content.ends_with(b"\n");
            let content = content.strip_suffix(b"\n").unwrap_or(content).to_vec();
            let mapped = match kind {
                GixLineKind::Context => {
                    let line = DiffLine {
                        kind: DiffLineKind::Context,
                        content,
                        missing_final_newline,
                        old_line: Some(old_line),
                        new_line: Some(new_line),
                    };
                    old_line = old_line.saturating_add(1);
                    new_line = new_line.saturating_add(1);
                    line
                }
                GixLineKind::Add => {
                    let line = DiffLine {
                        kind: DiffLineKind::Addition,
                        content,
                        missing_final_newline,
                        old_line: None,
                        new_line: Some(new_line),
                    };
                    new_line = new_line.saturating_add(1);
                    line
                }
                GixLineKind::Remove => {
                    let line = DiffLine {
                        kind: DiffLineKind::Removal,
                        content,
                        missing_final_newline,
                        old_line: Some(old_line),
                        new_line: None,
                    };
                    old_line = old_line.saturating_add(1);
                    line
                }
            };
            diff_lines.push(mapped);
        }
        self.hunks.push(DiffHunk {
            header: format!("{header}").into_bytes(),
            lines: diff_lines,
        });
        Ok(())
    }

    fn finish(self) -> Self::Out {
        self.hunks
    }
}

fn add_counts(
    stats: &mut HashMap<GitPath, (u64, u64)>,
    path: &GitPath,
    old: Option<&[u8]>,
    new: Option<&[u8]>,
) {
    let old = old.unwrap_or(b"");
    let new = new.unwrap_or(b"");
    if old == new {
        return;
    }
    let mut added = 0_u64;
    let mut deleted = 0_u64;
    for hunk in unified_hunks(old, new) {
        for line in hunk.lines {
            match line.kind {
                DiffLineKind::Addition => added = added.saturating_add(1),
                DiffLineKind::Removal => deleted = deleted.saturating_add(1),
                DiffLineKind::Context => {}
            }
        }
    }
    if added == 0 && deleted == 0 {
        return;
    }
    let entry = stats.entry(path.clone()).or_insert((0, 0));
    entry.0 = entry.0.saturating_add(added);
    entry.1 = entry.1.saturating_add(deleted);
}

fn peel_to_commit<'repo>(
    repo: &'repo gix::Repository,
    spec: &str,
) -> Result<gix::Commit<'repo>, GitBackendError> {
    repo.rev_parse_single(BStr::new(spec.as_bytes()))
        .map_err(gix_error)?
        .object()
        .map_err(gix_error)?
        .peel_to_commit()
        .map_err(gix_error)
}

fn peel_to_tree<'repo>(
    repo: &'repo gix::Repository,
    spec: &str,
) -> Result<gix::Tree<'repo>, GitBackendError> {
    repo.rev_parse_single(BStr::new(spec.as_bytes()))
        .map_err(gix_error)?
        .object()
        .map_err(gix_error)?
        .peel_to_tree()
        .map_err(gix_error)
}

fn tree_at<'repo>(
    repo: &'repo gix::Repository,
    oid: &str,
    path: &GitPath,
) -> Result<gix::Tree<'repo>, GitBackendError> {
    let tree = peel_to_tree(repo, oid)?;
    if path.0.is_empty() {
        return Ok(tree);
    }
    let entry = lookup_path(&tree, path)?
        .ok_or_else(|| GitBackendError::from_message("tree path not found"))?;
    repo.find_tree(entry.object_id()).map_err(gix_error)
}

fn lookup_path<'repo>(
    tree: &gix::Tree<'repo>,
    path: &GitPath,
) -> Result<Option<gix::object::tree::Entry<'repo>>, GitBackendError> {
    tree.lookup_entry(
        path.0
            .split(|byte| *byte == b'/')
            .filter(|component| !component.is_empty())
            .map(gix::bstr::BString::from),
    )
    .map_err(gix_error)
}

fn head_blob(repo: &gix::Repository, path: &GitPath) -> Result<Option<Vec<u8>>, GitBackendError> {
    let Ok(tree) = repo.head_tree() else {
        return Ok(None);
    };
    match lookup_path(&tree, path)? {
        Some(entry) if entry.mode().kind() != EntryKind::Tree => {
            blob_bytes(repo, entry.object_id()).map(Some)
        }
        _ => Ok(None),
    }
}

fn index_blob(repo: &gix::Repository, path: &GitPath) -> Result<Option<Vec<u8>>, GitBackendError> {
    let index = open_index(repo);
    let path_bstr = BStr::new(&path.0);
    let Some(entry) = index.entry_by_path(path_bstr) else {
        return Ok(None);
    };
    blob_bytes(repo, entry.id).map(Some)
}

fn worktree_bytes(
    repository: &git_domain::WorktreeRepository,
    path: &GitPath,
) -> Result<Option<Vec<u8>>, GitBackendError> {
    let full = worktree_path(repository, path)?;
    if full.is_symlink() {
        let target = std::fs::read_link(&full).map_err(gix_error)?;
        return Ok(Some(std::os::unix::ffi::OsStringExt::into_vec(
            target.into_os_string(),
        )));
    }
    match std::fs::read(&full) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(gix_error(error)),
    }
}

fn blob_bytes(
    repo: &gix::Repository,
    id: impl Into<gix::ObjectId>,
) -> Result<Vec<u8>, GitBackendError> {
    Ok(repo
        .find_object(id)
        .map_err(gix_error)?
        .peel_to_kind(gix::object::Kind::Blob)
        .map_err(gix_error)?
        .try_into_blob()
        .map_err(gix_error)?
        .data
        .clone())
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

fn ls_tree_mode(kind: EntryKind) -> String {
    match kind {
        EntryKind::Tree => "040000".into(),
        other => String::from_utf8_lossy(other.as_octal_str().as_bytes()).into_owned(),
    }
}

fn is_binary(bytes: &[u8]) -> bool {
    bytes.contains(&0)
}

fn is_binary_mode(mode: EntryMode) -> bool {
    matches!(mode.kind(), EntryKind::Commit)
}
