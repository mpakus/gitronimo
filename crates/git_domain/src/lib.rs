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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RefDecoration {
    pub name: Vec<u8>,
    pub target: String,
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
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Remote {
    pub name: GitPath,
    pub fetch_url: Vec<u8>,
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
    use super::{CommitIdentity, GraphState, HistoryCommit, layout_history_graph};

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
