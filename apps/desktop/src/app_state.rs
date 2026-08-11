//! Top-level shell state, small enums, and free helpers shared across views.
//!
//! Render code lives in `views/`; state-mutating logic lives in `main`.
//! This module owns the shared types so the rest of the desktop crate can
//! reference them without a circular dependency on `main.rs`.

use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
    process::Command,
    sync::{Arc, Mutex, mpsc::Receiver},
};

use git_cli::LoadedDiff;
use git_domain::{
    BlameLine, GitPath, GraphRow, GraphState, HistoryCommit, HistoryReference, HostedRepository,
    LfsEntry, PullRequestDetail, PullRequestSummary, RebaseTodoItem, RefDecoration, RefSnapshot,
    ReflogEntry, ServiceAccount, ServiceAuthState, StashEntry, SubmoduleEntry, TreeEntry,
    WorktreeEntry, WorktreeRepository,
};
use gpui::{FocusHandle, ListState, WindowAppearance};
use notify::RecommendedWatcher;
use ui_kit::Appearance;

pub(crate) const MINIMUM_PANE_WIDTH: f32 = 180.0;
pub(crate) const MAXIMUM_PANE_WIDTH: f32 = 440.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LastAction {
    Refresh,
}

pub(crate) struct OpenedRepository {
    pub repository: WorktreeRepository,
    pub recents: Vec<PathBuf>,
}

/// Which welcome-screen surface is active before a repository is opened.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum WelcomeShellView {
    /// Bookmarks tab — saved/local repositories.
    Repositories,
    Services,
    Workflow,
}

/// Lightweight Git metadata shown on the welcome detail panel.
#[derive(Clone, Debug, Default)]
pub(crate) struct WelcomeRepoSnapshot {
    pub branch: Option<String>,
    pub upstream: Option<String>,
    pub ahead: u32,
    pub behind: u32,
    pub changed_files: Option<usize>,
    pub remote_url: Option<String>,
    pub author_name: Option<String>,
    pub author_email: Option<String>,
    pub last_commit_subject: Option<String>,
    pub last_modified: Option<std::time::SystemTime>,
    pub available: bool,
}

pub(crate) enum ShellState {
    Welcome,
    Loading(PathBuf),
    Repository(WorktreeRepository),
    Error(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ThemeMode {
    System,
    Light,
    Dark,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RepositoryView {
    WorkingCopy,
    History,
    CommitDetail,
    Stashes,
    Remotes,
    Services,
    Settings,
    PullRequests,
    BranchesReview,
    Reflog,
    FileHistory,
    Blame,
    Compare,
    Tree,
    Worktrees,
    Submodules,
    Lfs,
    Rebase,
    Conflicts,
}

/// Which panel the Commit Detail view shows for the selected commit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum HistoryDetailMode {
    Changeset,
    Tree,
}

pub(crate) struct NetworkOperation {
    pub label: String,
    pub child: Option<git_cli::GitChild>,
    pub cancelled: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ForcePushState {
    Idle,
    AwaitingConfirmation,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ShortcutReferenceState {
    Hidden,
    Visible,
}

#[derive(Clone)]
pub(crate) enum RefContext {
    LocalBranch(String),
    RemoteBranch(String),
    Tag(String),
    Remote(String),
}

#[derive(Clone, Debug)]
pub(crate) enum TextPromptKind {
    BranchRename { current: String },
    CreateBranch { start: Option<String> },
    FileHistoryPath,
    BlamePath,
    CompareFrom,
    CompareTo { left: String },
}

#[derive(Clone, Copy)]
pub(crate) enum RefKind {
    LocalBranch,
    RemoteBranch,
    Tag,
}

impl RefKind {
    pub(crate) fn context(self, name: String) -> RefContext {
        match self {
            Self::LocalBranch => RefContext::LocalBranch(name),
            Self::RemoteBranch => RefContext::RemoteBranch(name),
            Self::Tag => RefContext::Tag(name),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mutation {
    StageSelected,
    UnstageSelected,
    StageAll,
    UnstageAll,
    DiscardSelected,
}

impl Mutation {
    pub(crate) fn needs_paths(self) -> bool {
        matches!(
            self,
            Self::StageSelected | Self::UnstageSelected | Self::DiscardSelected
        )
    }
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::StageSelected => "Stage selected",
            Self::UnstageSelected => "Unstage selected",
            Self::StageAll => "Stage all",
            Self::UnstageAll => "Unstage all",
            Self::DiscardSelected => "Discard selected",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum StashAction {
    Pop,
    Drop,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum OperationAction {
    Abort,
    Continue,
}

#[allow(clippy::struct_excessive_bools)]
pub(crate) struct GitronimoApp {
    pub focus_handle: FocusHandle,
    pub last_action: Option<LastAction>,
    pub appearance: Appearance,
    pub theme_mode: ThemeMode,
    pub sidebar_width: f32,
    pub state: ShellState,
    pub recents: Vec<PathBuf>,
    pub selected_recent: Option<usize>,
    pub welcome_snapshot: Option<WelcomeRepoSnapshot>,
    pub welcome_snapshot_path: Option<PathBuf>,
    pub welcome_snapshot_token: u64,
    pub welcome_list_snapshots: std::collections::HashMap<PathBuf, WelcomeRepoSnapshot>,
    pub welcome_list_snapshot_token: u64,
    pub welcome_shell_view: WelcomeShellView,
    pub repositories_grouped: bool,
    pub welcome_repo_search: String,
    pub worktree_file_search: String,
    #[allow(dead_code)]
    pub search_focus_handle: FocusHandle,
    pub commit_subject_focused: bool,
    pub network_progress: f32,
    pub last_network_result: Option<String>,
    pub activity: String,
    pub working_copy: Option<git_domain::WorktreeStatus>,
    pub worktree_show_all_files: bool,
    pub tracked_files: Vec<git_domain::GitPath>,
    pub refs: RefSnapshot,
    pub expanded_ref_groups: BTreeSet<String>,
    pub ref_context: Option<RefContext>,
    pub selected_paths: Vec<GitPath>,
    pub last_selected_path_index: Option<usize>,
    pub context_path: Option<GitPath>,
    pub loaded_diff: Option<LoadedDiff>,
    pub selected_diff: Option<(GitPath, bool)>,
    pub selected_diff_lines: Vec<(usize, usize)>,
    pub pending_line_discard: Option<(GitPath, Vec<(usize, usize)>)>,
    pub pending_hunk_discard: Option<(GitPath, usize)>,
    pub pending_discard: Option<Vec<GitPath>>,
    pub pending_stash_action: Option<StashAction>,
    pub pending_operation_action: Option<OperationAction>,
    pub pending_branch_delete: Option<String>,
    pub pending_text_prompt: Option<TextPromptKind>,
    pub text_prompt_value: String,
    pub selected_branch_review: Option<String>,
    pub branches_review_show_all: bool,
    pub force_push_state: ForcePushState,
    pub shortcut_reference_state: ShortcutReferenceState,
    pub commit_subject: String,
    pub commit_body: String,
    pub commit_amend: bool,
    pub commit_sign_off: bool,
    pub author_identity: String,
    pub repository_view: RepositoryView,
    pub navigation_back: Vec<RepositoryView>,
    pub navigation_forward: Vec<RepositoryView>,
    pub came_from_welcome: bool,
    pub history: Vec<HistoryCommit>,
    pub history_rows: Vec<GraphRow>,
    pub history_state: GraphState,
    pub history_reference: HistoryReference,
    pub history_next: Option<String>,
    pub history_decorations: Vec<RefDecoration>,
    pub selected_history: Option<usize>,
    pub history_search: String,
    pub history_list_state: ListState,
    pub history_paths: Vec<GitPath>,
    pub history_diff: Option<LoadedDiff>,
    pub history_selection_token: u64,
    pub history_load_token: u64,
    pub history_reveal_oid: Option<String>,
    pub history_detail_mode: HistoryDetailMode,
    pub stashes: Vec<StashEntry>,
    pub stashes_load_token: u64,
    pub selected_stash: Option<usize>,
    pub pending_stash_action_ref: Option<(StashAction, String, String)>,
    pub reflog: Vec<ReflogEntry>,
    pub reflog_load_token: u64,
    pub selected_reflog: Option<usize>,
    pub file_history: Vec<HistoryCommit>,
    pub file_history_path: String,
    pub file_history_load_token: u64,
    pub blame: Vec<BlameLine>,
    pub blame_path: String,
    pub blame_load_token: u64,
    pub compare_diff: Option<git_cli::LoadedDiff>,
    pub compare_left: String,
    pub compare_right: String,
    pub compare_load_token: u64,
    pub tree: Vec<TreeEntry>,
    pub tree_oid: String,
    pub tree_path: Vec<GitPath>,
    pub tree_blob: Option<Vec<u8>>,
    pub tree_blob_path: Option<GitPath>,
    pub tree_load_token: u64,
    pub worktrees: Vec<WorktreeEntry>,
    pub worktrees_load_token: u64,
    pub submodules: Vec<SubmoduleEntry>,
    pub submodules_load_token: u64,
    pub lfs: Vec<LfsEntry>,
    pub lfs_load_token: u64,
    pub service_auth_state: ServiceAuthState,
    pub service_account: Option<ServiceAccount>,
    pub hosted_repositories: Vec<HostedRepository>,
    pub selected_hosted_repository: Option<usize>,
    pub services_load_token: u64,
    pub pull_requests: Vec<PullRequestSummary>,
    pub selected_pull_request: Option<usize>,
    pub pull_request_detail: Option<PullRequestDetail>,
    pub pull_request_repository: Option<HostedRepository>,
    pub pull_requests_load_token: u64,
    pub pull_request_detail_token: u64,
    pub rebase_plan: Vec<RebaseTodoItem>,
    pub rebase_plan_load_token: u64,
    pub conflict_path: Option<GitPath>,
    pub conflict_content: Option<Vec<u8>>,
    pub mutation_in_flight: bool,
    pub network_operation: Option<Arc<Mutex<NetworkOperation>>>,
    pub watcher: Option<RecommendedWatcher>,
    pub watch_events: Option<Receiver<()>>,
    pub store: app_core::RecentRepositoryStore,
    pub diagnostics: String,
    pub subscriptions: Vec<gpui::Subscription>,
    pub column_width: f32,
    pub welcome_search_input: gpui::Entity<crate::views::single_line_input::SingleLineInput>,
    pub worktree_search_input: gpui::Entity<crate::views::single_line_input::SingleLineInput>,
    pub commit_subject_input: gpui::Entity<crate::views::single_line_input::SingleLineInput>,
    pub commit_body_input: gpui::Entity<crate::views::single_line_input::SingleLineInput>,
    pub repo_description_input: gpui::Entity<crate::views::single_line_input::SingleLineInput>,
    pub text_prompt_input: gpui::Entity<crate::views::single_line_input::SingleLineInput>,
    pub show_quick_open: bool,
    pub commit_options_expanded: bool,
    pub user_repo_description: String,
    pub last_commit_summary: Option<String>,
    pub file_diff_stats: std::collections::HashMap<git_domain::GitPath, (usize, usize)>,
}

impl GitronimoApp {
    pub(crate) fn has_commit_draft(&self) -> bool {
        !self.commit_subject.trim().is_empty() || !self.commit_body.trim().is_empty()
    }

    #[allow(dead_code)]
    pub(crate) fn active_search_query(&self) -> &str {
        match &self.state {
            ShellState::Welcome => self.welcome_repo_search.as_str(),
            ShellState::Repository(_) => self.worktree_file_search.as_str(),
            _ => "",
        }
    }
}

pub(crate) fn network_failure_message(label: &str, error: &str) -> String {
    let error = error.to_lowercase();
    if error.contains("authentication")
        || error.contains("permission denied")
        || error.contains("could not read username")
    {
        format!(
            "{label} failed: authentication was rejected. Check your Git credentials or SSH key."
        )
    } else if error.contains("non-fast-forward") || error.contains("fetch first") {
        format!("{label} failed: the remote has newer commits. Pull or rebase, then push again.")
    } else {
        format!("{label} failed. Check the configured remote and repository access.")
    }
}

pub(crate) fn git_failure_message(label: &str, error: &str) -> String {
    if error.to_lowercase().contains("index.lock") {
        format!(
            "{label} could not run because Git's index is locked. Check that no Git process is still running; if none is, inspect .git/index.lock before removing it manually."
        )
    } else {
        format!("{label} failed: {error}")
    }
}

pub(crate) fn repository_is_available(repository: &WorktreeRepository) -> bool {
    repository.worktree_root.is_dir() && repository.git_dir.is_dir()
}

pub(crate) fn repository_unavailable_message(repository: &WorktreeRepository) -> String {
    format!(
        "{} is no longer available. Restore the repository folder, then open it again.",
        repository.worktree_root.display()
    )
}

pub(crate) fn appearance_from_window(appearance: WindowAppearance) -> Appearance {
    match appearance {
        WindowAppearance::Light | WindowAppearance::VibrantLight => Appearance::Light,
        WindowAppearance::Dark | WindowAppearance::VibrantDark => Appearance::Dark,
    }
}

pub(crate) fn window_title(state: &ShellState, has_commit_draft: bool) -> String {
    let repository = match state {
        ShellState::Repository(repository) => repository
            .worktree_root
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("Repository"),
        ShellState::Loading(_) => "Opening repository",
        ShellState::Error(_) => "Repository error",
        ShellState::Welcome => return "Gitronimo".into(),
    };
    if matches!(state, ShellState::Repository(_)) {
        format!(
            "{repository} — Gitronimo{}",
            if has_commit_draft { " • Draft" } else { "" }
        )
    } else {
        format!("{repository} — Gitronimo")
    }
}

pub(crate) fn resize_width(width: f32) -> f32 {
    (width + 20.0).clamp(MINIMUM_PANE_WIDTH, MAXIMUM_PANE_WIDTH)
}

pub(crate) fn discard_selected(
    git: &git_cli::GitExecutable,
    repository: &WorktreeRepository,
    paths: &[GitPath],
) -> Result<(), git_cli::GitStatusError> {
    for path in paths {
        match git.discard_tracked_paths(repository, std::slice::from_ref(path)) {
            Ok(()) => {}
            Err(git_cli::GitStatusError::UntrackedDeletionRefused) => {
                move_to_trash(&repository.worktree_root, path)
                    .map_err(git_cli::GitStatusError::Io)?;
            }
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

fn move_to_trash(root: &Path, path: &GitPath) -> std::io::Result<()> {
    let target = eligible_trash_path(root, path)?;
    let output = Command::new("osascript")
        .args([
            "-e",
            "on run argv\ntell application \"Finder\" to delete POSIX file (item 1 of argv)\nend run",
        ])
        .arg(target)
        .output()?;
    output.status.success().then_some(()).ok_or_else(|| {
        std::io::Error::other(String::from_utf8_lossy(&output.stderr).trim().to_owned())
    })
}

pub(crate) fn eligible_trash_path(root: &Path, path: &GitPath) -> std::io::Result<PathBuf> {
    use std::{ffi::OsString, fs, os::unix::ffi::OsStringExt};

    if path.0.starts_with(b"/")
        || path.0.split(|byte| *byte == b'/').any(|part| part == b"..")
        || std::str::from_utf8(&path.0).is_err()
    {
        return Err(std::io::Error::other("unsupported path for Finder Trash"));
    }
    let target = root.join(OsString::from_vec(path.0.clone()));
    let metadata = fs::symlink_metadata(&target)?;
    if metadata.file_type().is_symlink() || (metadata.is_dir() && target.join(".git").exists()) {
        return Err(std::io::Error::other(
            "refusing symlink or nested repository",
        ));
    }
    Ok(target)
}
