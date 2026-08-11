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
#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
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

/// A history-changing Git operation paused in the repository awaiting a decision.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum InProgressOperation {
    /// No history-changing operation is in progress.
    #[default]
    None,
    /// `git merge` reached a conflict; `oid` is the branch being merged.
    Merge { oid: Option<Vec<u8>> },
    /// `git cherry-pick` reached a conflict; `oid` is the cherry-picked commit.
    CherryPick { oid: Option<Vec<u8>> },
    /// `git revert` reached a conflict; `oid` is the reverted commit.
    Revert { oid: Option<Vec<u8>> },
    /// `git rebase` is paused on a conflict or an instruction edit.
    Rebase,
}

/// A point-in-time snapshot of the refs a history-changing operation can move,
/// recorded before the operation runs so its start state can be restored or
/// described later. Contains no credentials.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RecoveryRecord {
    /// The pre-operation HEAD oid, or `None` for an unborn repository.
    pub old_head: Option<Vec<u8>>,
    /// The symbolic local branch HEAD points at, if any.
    pub head_name: Option<GitPath>,
    /// The pre-operation local branch tips; these are the refs history-changing
    /// operations can move.
    pub branch_tips: Vec<RecoveredBranchTip>,
}

/// One local branch tip captured in a [`RecoveryRecord`].
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RecoveredBranchTip {
    /// The full ref name, for example `refs/heads/main`.
    pub name: GitPath,
    /// The ref's current oid.
    pub oid: Vec<u8>,
}

/// The complete non-mutating working-copy state.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WorktreeStatus {
    pub branch: BranchStatus,
    pub stash_count: u32,
    pub operation: InProgressOperation,
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
    pub old_line: Option<u64>,
    pub new_line: Option<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiffLineKind {
    Context,
    Addition,
    Removal,
}

/// Parses a unified-diff hunk header, returning the old and new starting line
/// numbers. Both counts may be omitted, and a trailing `@@` section label is ignored.
///
/// ```text
/// @@ -old_start[,old_count] +new_start[,new_count] @@
/// ```
#[must_use]
pub fn parse_hunk_header(header: &[u8]) -> Option<(u64, u64)> {
    let rest = header.strip_prefix(b"@@ -")?;
    let (old_start, rest) = parse_u64_at(rest)?;
    let rest = if rest.first() == Some(&b',') {
        let (_, rest) = parse_u64_at(&rest[1..])?;
        rest
    } else {
        rest
    };
    let rest = rest.strip_prefix(b" +")?;
    let (new_start, rest) = parse_u64_at(rest)?;
    let rest = if rest.first() == Some(&b',') {
        let (_, rest) = parse_u64_at(&rest[1..])?;
        rest
    } else {
        rest
    };
    rest.starts_with(b" @@").then_some((old_start, new_start))
}

fn parse_u64_at(bytes: &[u8]) -> Option<(u64, &[u8])> {
    let digits = bytes
        .iter()
        .take_while(|byte| byte.is_ascii_digit())
        .count();
    if digits == 0 {
        return None;
    }
    let value = std::str::from_utf8(&bytes[..digits]).ok()?.parse().ok()?;
    Some((value, &bytes[digits..]))
}

/// Builds a unified-diff patch containing only the selected change lines of one
/// hunk, keeping the context lines that anchor them and recomputing each `@@`
/// header. The hunk is split at unselected change lines so a sub-hunk never
/// spans a change that is left out (which would shift later positions).
///
/// `selected` holds indexes into `hunk.lines`; context lines in the selection are
/// ignored because they carry no change to apply. Returns `None` when no change
/// line is selected or the hunk header cannot be parsed.
#[must_use]
pub fn selected_lines_patch(hunk: &DiffHunk, selected: &[usize]) -> Option<Vec<u8>> {
    let (old_start, new_start) = parse_hunk_header(&hunk.header)?;
    let mut old = old_start;
    let mut new = new_start;
    let mut segments: Vec<Segment> = Vec::new();
    let mut current = Segment::default();

    for (index, line) in hunk.lines.iter().enumerate() {
        let is_selected = selected.contains(&index);
        match line.kind {
            DiffLineKind::Context => {
                if current.lines.is_empty() {
                    current.old_start = old;
                    current.new_start = new;
                }
                current.lines.push(line);
                current.old_count += 1;
                current.new_count += 1;
                old += 1;
                new += 1;
            }
            DiffLineKind::Addition => {
                if is_selected {
                    if current.lines.is_empty() {
                        current.old_start = old;
                        current.new_start = new;
                    }
                    current.lines.push(line);
                    current.new_count += 1;
                    current.saw_change = true;
                } else {
                    push_segment(&mut segments, &mut current);
                }
                new += 1;
            }
            DiffLineKind::Removal => {
                if is_selected {
                    if current.lines.is_empty() {
                        current.old_start = old;
                        current.new_start = new;
                    }
                    current.lines.push(line);
                    current.old_count += 1;
                    current.saw_change = true;
                } else {
                    push_segment(&mut segments, &mut current);
                }
                old += 1;
            }
        }
    }
    push_segment(&mut segments, &mut current);

    if segments.is_empty() {
        return None;
    }
    let mut patch = Vec::new();
    for segment in segments {
        patch.extend_from_slice(
            format!(
                "@@ -{},{} +{},{} @@\n",
                segment.old_start, segment.old_count, segment.new_start, segment.new_count
            )
            .as_bytes(),
        );
        for line in segment.lines {
            let prefix = match line.kind {
                DiffLineKind::Context => b' ',
                DiffLineKind::Addition => b'+',
                DiffLineKind::Removal => b'-',
            };
            patch.push(prefix);
            patch.extend_from_slice(&line.content);
            patch.push(b'\n');
            if line.missing_final_newline {
                patch.extend_from_slice(b"\\ No newline at end of file\n");
            }
        }
    }
    Some(patch)
}

#[derive(Default)]
struct Segment<'a> {
    old_start: u64,
    new_start: u64,
    old_count: u64,
    new_count: u64,
    lines: Vec<&'a DiffLine>,
    saw_change: bool,
}

fn push_segment<'a>(segments: &mut Vec<Segment<'a>>, current: &mut Segment<'a>) {
    if current.saw_change {
        segments.push(std::mem::take(current));
    } else {
        current.lines.clear();
        current.old_count = 0;
        current.new_count = 0;
    }
    current.saw_change = false;
}

/// A bounded request for commit history.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HistoryRequest {
    pub reference: HistoryReference,
    pub before: Option<String>,
    pub limit: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HistoryReference {
    Current,
    All,
    Named(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HistoryPage {
    pub commits: Vec<HistoryCommit>,
    pub next_before: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HistoryCommit {
    pub oid: String,
    pub parents: Vec<String>,
    pub author: CommitIdentity,
    pub committer: CommitIdentity,
    pub subject: Vec<u8>,
    pub body: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommitIdentity {
    pub name: Vec<u8>,
    pub email: Vec<u8>,
    pub timestamp: i64,
}

/// A bounded request for a ref's reflog, newest entry first.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReflogRequest {
    /// The ref whose reflog to read; `None` reads HEAD's reflog.
    pub reference: Option<String>,
    pub limit: usize,
}

/// One entry in a ref's reflog, newest first.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReflogEntry {
    /// The oid the ref pointed at before this entry, when the reflog chain
    /// provides one.
    pub old_oid: Option<Vec<u8>>,
    /// The oid the ref pointed at after this entry.
    pub new_oid: Vec<u8>,
    /// Git's selector for this entry, for example `HEAD@{2}`.
    pub selector: String,
    pub identity: CommitIdentity,
    /// Git's reflog message, for example `commit: Fix the flaky test`.
    pub subject: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RefDecoration {
    pub name: Vec<u8>,
    pub target: String,
}

/// A bounded request for the commit history of a single tracked path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileHistoryRequest {
    pub path: GitPath,
    pub limit: usize,
}

/// One source line with the commit that last touched it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlameLine {
    /// The oid of the commit that introduced the line.
    pub oid: Vec<u8>,
    pub author: CommitIdentity,
    /// The line content, without the trailing newline.
    pub content: Vec<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TreeEntryKind {
    Blob,
    Tree,
    Commit,
}

/// One entry in a commit's tree at a directory level.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TreeEntry {
    pub name: GitPath,
    pub kind: TreeEntryKind,
    pub oid: Vec<u8>,
    pub mode: String,
}

/// One linked worktree managed by the repository.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorktreeEntry {
    pub path: GitPath,
    /// The checked-out commit oid.
    pub head: Vec<u8>,
    /// The checked-out branch, `None` for a detached HEAD.
    pub branch: Option<GitPath>,
    /// Whether the worktree's files differ from HEAD.
    pub dirty: bool,
    /// Whether this is the main working directory.
    pub main: bool,
}

/// One submodule registered in `.gitmodules`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SubmoduleEntry {
    pub path: GitPath,
    /// Git's status flag: `-` uninitialized, `+` checked out but differing
    /// from the index, `U` merge conflicts, ` ` clean.
    pub flag: u8,
    /// The gitlink oid recorded in the index.
    pub oid: Vec<u8>,
    /// Git's `(describe)` suffix, when present.
    pub description: String,
}

/// One verb in an interactive rebase todo.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RebaseAction {
    Pick,
    Reword,
    Edit,
    Squash,
    Fixup,
    Drop,
    Exec,
    /// `label`, `reset`, `merge`, `break`, and anything else Git accepts.
    Other(String),
}

impl RebaseAction {
    /// The verb Git writes into the todo, and the app echoes back on save.
    #[must_use]
    pub fn verb(&self) -> &str {
        match self {
            RebaseAction::Pick => "pick",
            RebaseAction::Reword => "reword",
            RebaseAction::Edit => "edit",
            RebaseAction::Squash => "squash",
            RebaseAction::Fixup => "fixup",
            RebaseAction::Drop => "drop",
            RebaseAction::Exec => "exec",
            RebaseAction::Other(verb) => verb,
        }
    }

    /// The next action in the plan-editor cycle.
    #[must_use]
    pub fn next(&self) -> RebaseAction {
        match self {
            RebaseAction::Pick => RebaseAction::Reword,
            RebaseAction::Reword => RebaseAction::Edit,
            RebaseAction::Edit => RebaseAction::Squash,
            RebaseAction::Squash => RebaseAction::Fixup,
            RebaseAction::Fixup => RebaseAction::Drop,
            RebaseAction::Drop => RebaseAction::Pick,
            other => other.clone(),
        }
    }

    /// The action verb when parsing a todo line.
    #[must_use]
    pub fn from_verb(verb: &[u8]) -> RebaseAction {
        match verb {
            b"pick" | b"p" => RebaseAction::Pick,
            b"reword" | b"r" => RebaseAction::Reword,
            b"edit" | b"e" => RebaseAction::Edit,
            b"squash" | b"s" => RebaseAction::Squash,
            b"fixup" | b"f" => RebaseAction::Fixup,
            b"drop" | b"d" => RebaseAction::Drop,
            b"exec" | b"x" => RebaseAction::Exec,
            other => RebaseAction::Other(String::from_utf8_lossy(other).into_owned()),
        }
    }
}

/// One line of an interactive rebase plan.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RebaseTodoItem {
    pub action: RebaseAction,
    /// Everything after the verb on the original line, kept verbatim so a
    /// saved plan round-trips (the oid for pick/reword/edit/squash/fixup/drop,
    /// the command for exec, the label for label/reset).
    pub arguments: String,
}

/// Which side of a conflicted file to keep.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConflictSide {
    /// The branch being rebased onto, or the current branch during a merge.
    Ours,
    /// The commit being applied, or the other branch during a merge.
    Theirs,
}

/// Git's `%G?` verdict for a commit signature.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CommitSignatureStatus {
    /// A valid signature.
    Good,
    /// A bad signature.
    Bad,
    /// A good signature whose key is unknown.
    Unknown,
    /// No signature.
    None,
    /// A good signature whose key has expired.
    Expired,
    /// A good signature made by an expired key.
    GoodExpired,
    /// A signature by a revoked key.
    Revoked,
    /// An error while checking.
    Error,
    /// Any other verdict Git emits.
    Other(String),
}

impl CommitSignatureStatus {
    #[must_use]
    pub fn label(&self) -> &str {
        match self {
            CommitSignatureStatus::Good => "good",
            CommitSignatureStatus::Bad => "bad",
            CommitSignatureStatus::Unknown => "unknown key",
            CommitSignatureStatus::None => "unsigned",
            CommitSignatureStatus::Expired => "expired",
            CommitSignatureStatus::GoodExpired => "good, expired key",
            CommitSignatureStatus::Revoked => "revoked key",
            CommitSignatureStatus::Error => "error",
            CommitSignatureStatus::Other(verdict) => verdict,
        }
    }
}

/// The signature status and signer of one commit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommitSignature {
    pub status: CommitSignatureStatus,
    /// The `%GS` signer line, empty for unsigned commits.
    pub signer: String,
}

/// A non-mutating snapshot of refs and configured remotes.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RefSnapshot {
    pub local_branches: Vec<NamedRef>,
    pub remote_branches: Vec<NamedRef>,
    pub tags: Vec<NamedRef>,
    pub remotes: Vec<Remote>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NamedRef {
    pub name: GitPath,
    pub target: String,
    /// Short upstream ref name when configured (local branches only).
    pub upstream: Option<String>,
    /// Commits on this branch not in upstream.
    pub ahead: u32,
    /// Commits on upstream not in this branch.
    pub behind: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Remote {
    pub name: GitPath,
    pub fetch_url: Vec<u8>,
}

/// One entry in the stash reflog, e.g. `stash@{0}`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StashEntry {
    /// The git selector, e.g. `stash@{0}`.
    pub reference: String,
    /// The stash commit oid.
    pub oid: String,
    /// The stash subject (`%gs`), typically "WIP on <branch>: <oid> <subject>".
    pub subject: Vec<u8>,
}

/// A changed Git LFS path reported by `git lfs status --porcelain`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LfsEntry {
    /// The index-side status column, or a space when unchanged.
    pub index_status: u8,
    /// The worktree-side status column, or a space when unchanged.
    pub worktree_status: u8,
    /// The path bytes after the two status columns and separator.
    pub path: GitPath,
}

/// Authentication state for a configured hosting service account.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ServiceAuthState {
    SignedOut,
    Loading,
    Connected,
    Expired,
    RateLimited,
    Error(String),
}

/// Non-secret identity metadata returned by a hosting provider.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServiceAccount {
    pub provider: String,
    pub login: String,
    pub display_name: Option<String>,
}

/// A repository hosted by a provider. Credentials are intentionally absent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostedRepository {
    pub id: u64,
    pub owner: String,
    pub name: String,
    pub full_name: String,
    pub clone_url: String,
    pub ssh_url: Option<String>,
    pub private: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PullRequestState {
    Open,
    Closed,
    Merged,
    Other(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MergeMethod {
    Merge,
    Squash,
    Rebase,
}

impl MergeMethod {
    #[must_use]
    pub fn api_name(self) -> &'static str {
        match self {
            Self::Merge => "merge",
            Self::Squash => "squash",
            Self::Rebase => "rebase",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PullRequestSummary {
    pub number: u64,
    pub title: String,
    pub author: String,
    pub updated_at: String,
    pub state: PullRequestState,
    pub head_ref: String,
    pub base_ref: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PullRequestFile {
    pub path: String,
    pub additions: u64,
    pub deletions: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PullRequestComment {
    pub author: String,
    pub body: String,
    pub created_at: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PullRequestDetail {
    pub summary: PullRequestSummary,
    pub body: String,
    pub files: Vec<PullRequestFile>,
    pub comments: Vec<PullRequestComment>,
}

/// Lane state carried between bounded history pages.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GraphState {
    pub lanes: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GraphRow {
    pub lane: usize,
    pub parent_lanes: Vec<usize>,
    pub octopus: bool,
}

/// Lays out a history page while retaining unresolved parent lanes for the next page.
#[must_use]
pub fn layout_history_graph(commits: &[HistoryCommit], state: &mut GraphState) -> Vec<GraphRow> {
    commits
        .iter()
        .map(|commit| {
            let lane = state
                .lanes
                .iter()
                .position(|oid| oid == &commit.oid)
                .unwrap_or_else(|| {
                    state.lanes.insert(0, commit.oid.clone());
                    0
                });
            let mut parent_lanes = Vec::new();
            if let Some(first_parent) = commit.parents.first() {
                state.lanes[lane].clone_from(first_parent);
                parent_lanes.push(lane);
                for parent in commit.parents.iter().skip(1) {
                    let parent_lane = state
                        .lanes
                        .iter()
                        .position(|oid| oid == parent)
                        .unwrap_or_else(|| {
                            let next = lane + parent_lanes.len();
                            state.lanes.insert(next, parent.clone());
                            next
                        });
                    parent_lanes.push(parent_lane);
                }
            } else {
                state.lanes.remove(lane);
            }
            GraphRow {
                lane,
                parent_lanes,
                octopus: commit.parents.len() > 2,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{
        CommitIdentity, DiffHunk, DiffLine, DiffLineKind, GraphState, HistoryCommit,
        layout_history_graph, parse_hunk_header, selected_lines_patch,
    };

    fn diff_line(kind: DiffLineKind, content: &str) -> DiffLine {
        DiffLine {
            kind,
            content: content.as_bytes().to_vec(),
            missing_final_newline: false,
            old_line: None,
            new_line: None,
        }
    }

    #[test]
    fn hunk_headers_parse_with_and_without_counts_and_labels() {
        assert_eq!(parse_hunk_header(b"@@ -1,3 +1,4 @@"), Some((1, 1)));
        assert_eq!(parse_hunk_header(b"@@ -1 +1 @@"), Some((1, 1)));
        assert_eq!(parse_hunk_header(b"@@ -9,2 +7,4 @@ fn run()"), Some((9, 7)));
        assert_eq!(parse_hunk_header(b"@@ -0,0 +1,2 @@"), Some((0, 1)));
        assert_eq!(parse_hunk_header(b"plain text"), None);
        assert_eq!(parse_hunk_header(b"@@ -a +1 @@"), None);
    }

    fn sample_hunk() -> DiffHunk {
        DiffHunk {
            header: b"@@ -1,5 +1,5 @@".to_vec(),
            lines: vec![
                diff_line(DiffLineKind::Context, "alpha"),
                diff_line(DiffLineKind::Removal, "beta"),
                diff_line(DiffLineKind::Addition, "beta changed"),
                diff_line(DiffLineKind::Context, "gamma"),
                diff_line(DiffLineKind::Context, "delta"),
            ],
        }
    }

    #[test]
    fn partial_patch_keeps_context_and_recomputes_the_header() {
        let hunk = sample_hunk();
        let patch = selected_lines_patch(&hunk, &[2]).expect("addition should be selected");
        let text = String::from_utf8(patch).expect("patch should be UTF-8");
        assert_eq!(text, "@@ -3,2 +2,3 @@\n+beta changed\n gamma\n delta\n");
    }

    #[test]
    fn partial_patch_splits_at_unselected_changes() {
        let hunk = DiffHunk {
            header: b"@@ -1,5 +1,6 @@".to_vec(),
            lines: vec![
                diff_line(DiffLineKind::Context, "one"),
                diff_line(DiffLineKind::Addition, "ADD A"),
                diff_line(DiffLineKind::Addition, "SKIP B"),
                diff_line(DiffLineKind::Addition, "ADD C"),
                diff_line(DiffLineKind::Context, "two"),
            ],
        };
        let patch =
            selected_lines_patch(&hunk, &[1, 3]).expect("selected additions should be kept");
        let text = String::from_utf8(patch).expect("patch should be UTF-8");
        assert_eq!(
            text,
            "@@ -1,1 +1,2 @@\n one\n+ADD A\n@@ -2,1 +4,2 @@\n+ADD C\n two\n"
        );
    }

    #[test]
    fn partial_patch_can_select_a_removal() {
        let hunk = sample_hunk();
        let patch = selected_lines_patch(&hunk, &[1]).expect("removal should be selected");
        let text = String::from_utf8(patch).expect("patch should be UTF-8");
        assert_eq!(text, "@@ -1,2 +1,1 @@\n alpha\n-beta\n");
    }

    #[test]
    fn partial_patch_can_select_multiple_change_lines() {
        let hunk = sample_hunk();
        let patch = selected_lines_patch(&hunk, &[1, 2]).expect("changes should be selected");
        let text = String::from_utf8(patch).expect("patch should be UTF-8");
        assert_eq!(
            text,
            "@@ -1,4 +1,4 @@\n alpha\n-beta\n+beta changed\n gamma\n delta\n"
        );
    }

    #[test]
    fn partial_patch_ignores_context_selection_and_empty_selection() {
        let hunk = sample_hunk();
        assert!(selected_lines_patch(&hunk, &[0]).is_none());
        assert!(selected_lines_patch(&hunk, &[]).is_none());
        assert!(selected_lines_patch(&hunk, &[0, 3, 4]).is_none());
    }

    #[test]
    fn partial_patch_preserves_final_newline_markers() {
        let mut hunk = sample_hunk();
        hunk.lines[4].missing_final_newline = true;
        let patch = selected_lines_patch(&hunk, &[2]).expect("addition should be selected");
        let text = String::from_utf8(patch).expect("patch should be UTF-8");
        assert!(
            text.ends_with(" delta\n\\ No newline at end of file\n"),
            "final newline marker should survive, got {text:?}"
        );
    }

    fn commit(oid: &str, parents: &[&str]) -> HistoryCommit {
        let identity = CommitIdentity {
            name: b"Test".to_vec(),
            email: b"test@example.invalid".to_vec(),
            timestamp: 0,
        };
        HistoryCommit {
            oid: oid.into(),
            parents: parents.iter().map(|parent| (*parent).into()).collect(),
            author: identity.clone(),
            committer: identity,
            subject: Vec::new(),
            body: Vec::new(),
        }
    }

    #[test]
    fn graph_layout_preserves_linear_branch_merge_and_page_lanes() {
        let mut state = GraphState::default();
        let rows = layout_history_graph(
            &[
                commit("merge", &["main", "topic"]),
                commit("main", &["base"]),
                commit("topic", &["base"]),
                commit("base", &[]),
            ],
            &mut state,
        );
        assert_eq!(
            rows.iter().map(|row| row.lane).collect::<Vec<_>>(),
            vec![0, 0, 1, 0]
        );
        assert_eq!(rows[0].parent_lanes, vec![0, 1]);
        let mut paged_state = GraphState::default();
        let first = layout_history_graph(&[commit("head", &["parent"])], &mut paged_state);
        let second = layout_history_graph(&[commit("parent", &[])], &mut paged_state);
        assert_eq!((first[0].lane, second[0].lane), (0, 0));
        assert!(
            layout_history_graph(
                &[commit("octopus", &["a", "b", "c"])],
                &mut GraphState::default()
            )[0]
            .octopus
        );
    }
}
