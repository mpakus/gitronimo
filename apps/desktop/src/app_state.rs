//! Top-level shell state, small enums, and free helpers shared across views.
//!
//! Render code lives in `views/`; state-mutating logic lives in `main`.
//! This module owns the shared types so the rest of the desktop crate can
//! reference them without a circular dependency on `main.rs`.

use std::{
    collections::{BTreeSet, VecDeque},
    path::{Path, PathBuf},
    process::Command,
    sync::{Arc, Mutex, atomic::AtomicBool, mpsc::Receiver},
    time::SystemTime,
};

use git_cli::{LoadedDiff, redact_git_text};
use git_domain::{
    BlameLine, GitPath, GraphRow, GraphState, HistoryCommit, HistoryReference, HostedRepository,
    LfsEntry, PullRequestDetail, PullRequestSummary, RebaseTodoItem, RefDecoration, RefSnapshot,
    ReflogEntry, ServiceAccount, ServiceAuthState, StashEntry, SubmoduleEntry, TreeEntry,
    WorktreeEntry, WorktreeRepository,
};
use gpui::{FocusHandle, ListState, WindowAppearance};
use notify::RecommendedWatcher;
use ui_kit::{Appearance, ThemeColors};

pub(crate) const MINIMUM_PANE_WIDTH: f32 = 180.0;
pub(crate) const MAXIMUM_PANE_WIDTH: f32 = 440.0;
pub(crate) const DEFAULT_SIDEBAR_WIDTH: f32 = 220.0;
pub(crate) const MINIMUM_LIST_PANE_WIDTH: f32 = 200.0;
pub(crate) const MAXIMUM_LIST_PANE_WIDTH: f32 = 600.0;
pub(crate) const DEFAULT_LIST_PANE_WIDTH: f32 = 400.0;
pub(crate) const ACTIVITY_LOG_CAPACITY: usize = 100;

/// Severity / role of an activity line for the status bar and history popup.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ActivityKind {
    Success,
    Error,
    Progress,
    Confirm,
    Info,
}

/// One status / error / notification line retained for the activity history popup.
#[derive(Clone, Debug)]
pub(crate) struct ActivityLogEntry {
    pub message: String,
    pub kind: ActivityKind,
    pub at: SystemTime,
}

/// Classifies status-line copy for color and history filtering.
pub(crate) fn classify_activity(message: &str) -> ActivityKind {
    let lower = message.to_lowercase();
    if lower.contains("failed")
        || lower.contains("unable")
        || lower.contains("could not")
        || lower.contains("error:")
        || lower.contains("refused")
    {
        ActivityKind::Error
    } else if lower.starts_with("confirm ")
        || lower.contains("confirm deletion")
        || lower.contains("confirm discard")
        || lower.contains("cancelled")
        || lower.contains("review deletion")
    {
        ActivityKind::Confirm
    } else if lower.contains("complete")
        || lower.contains("refreshed")
        || lower.contains("opened")
        || lower.starts_with("pinned ")
        || lower.starts_with("unpinned ")
        || lower.starts_with("archived ")
        || lower.starts_with("unarchived ")
        || lower.starts_with("update installed")
        || lower.ends_with(" saved.")
        || lower.ends_with(" created.")
        || lower.ends_with(" deleted.")
        || lower.ends_with(" renamed.")
    {
        ActivityKind::Success
    } else if message.ends_with('…') || lower.contains("in progress") {
        ActivityKind::Progress
    } else {
        ActivityKind::Info
    }
}

/// Working-copy refresh chatter that would drown out push/error/confirm lines.
pub(crate) fn is_working_copy_refresh_noise(message: &str) -> bool {
    message.starts_with("Refreshing working copy") || message.starts_with("Working copy refreshed")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LastAction {
    Refresh,
}

pub(crate) struct OpenedRepository {
    pub repository: WorktreeRepository,
    pub recents: Vec<PathBuf>,
    pub git_fallback: Option<String>,
}

/// Which welcome-screen surface is active before a repository is opened.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum WelcomeShellView {
    /// Bookmarks tab — saved/local repositories.
    Repositories,
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
    Settings,
    Workflow,
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
    pub interrupt: Arc<AtomicBool>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ForcePushState {
    Idle,
    AwaitingConfirmation,
}

/// Modal confirmation for destructive or blocked Git actions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum AppConfirmDialog {
    /// Safe delete refused because the branch is not fully merged.
    ForceDeleteBranch { branch: String },
    /// Hard reset discards uncommitted work and moves HEAD.
    HardReset { oid: String },
    /// Create a revert commit for the selected history commit.
    RevertCommit { oid: String },
    /// Drop a commit from the current branch history via rebase.
    DropCommit { oid: String },
    /// Finish the current workflow topic into its configured base(s).
    FinishTopic {
        branch: String,
        label: String,
        targets: String,
        strategy: String,
    },
    /// Replace this GitRonimo.app with a verified GitHub release zip.
    InstallUpdate { version: String },
}

impl AppConfirmDialog {
    pub(crate) fn title(&self) -> String {
        match self {
            Self::ForceDeleteBranch { .. } => "Could Not Delete Branch".into(),
            Self::HardReset { .. } => "Hard Reset".into(),
            Self::RevertCommit { .. } => "Revert Commit".into(),
            Self::DropCommit { .. } => "Delete Commit".into(),
            Self::FinishTopic { .. } => "Finish Topic Branch".into(),
            Self::InstallUpdate { .. } => "Install Update".into(),
        }
    }

    pub(crate) fn body(&self) -> String {
        match self {
            Self::ForceDeleteBranch { branch } => format!(
                "The branch \"{branch}\" contains unmerged changes. Do you really want to delete it?"
            ),
            Self::HardReset { oid } => {
                let short = short_commit_label(oid);
                format!(
                    "Reset HEAD hard to \"{short}\"? Uncommitted changes and commits after this point will be discarded."
                )
            }
            Self::RevertCommit { oid } => {
                let short = short_commit_label(oid);
                format!("Create a revert commit for \"{short}\"?")
            }
            Self::DropCommit { oid } => {
                let short = short_commit_label(oid);
                format!(
                    "Delete \"{short}\" from the current branch? This rewrites history via rebase."
                )
            }
            Self::FinishTopic {
                branch,
                label,
                targets,
                strategy,
            } => format!(
                "Finish {label} \"{branch}\" into {targets} using {strategy}? The topic branch is deleted after a successful finish."
            ),
            Self::InstallUpdate { version } => format!(
                "Download and install GitRonimo {version}? The zip is verified with SHA-256 and Gatekeeper before this app is replaced. Quit and reopen GitRonimo afterward."
            ),
        }
    }

    pub(crate) fn cancel_label() -> &'static str {
        "Cancel"
    }

    pub(crate) fn confirm_label(&self) -> &'static str {
        match self {
            Self::ForceDeleteBranch { .. } | Self::DropCommit { .. } => "Delete",
            Self::HardReset { .. } => "Reset",
            Self::RevertCommit { .. } => "Revert",
            Self::FinishTopic { .. } => "Finish",
            Self::InstallUpdate { .. } => "Install",
        }
    }
}

fn short_commit_label(oid: &str) -> String {
    oid.chars().take(8).collect()
}

/// Selected History commit for the right-click context menu.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CommitContext {
    pub oid: String,
    pub short_oid: String,
    pub index: usize,
    pub subject: String,
    pub is_head: bool,
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

/// Flyout opened from a ▸ item inside the ref context menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RefContextSubmenu {
    PushTo,
    TrackUpstream,
}

#[derive(Clone, Debug)]
pub(crate) enum TextPromptKind {
    BranchRename {
        current: String,
    },
    CreateBranch {
        start: Option<String>,
    },
    StartWorkflowTopic {
        prefix: String,
        start: String,
    },
    CreateTag {
        start: String,
    },
    /// Save stash or snapshot: prompt value is the message; options carry untracked + paths.
    CreateStash {
        snapshot: bool,
        include_untracked: bool,
        paths: Vec<GitPath>,
    },
    /// Branch name; creates a branch from the named stash.
    StashBranch {
        reference: String,
    },
    FileHistoryPath,
    BlamePath,
    CompareFrom,
    CompareTo {
        left: String,
    },
    DropCommit,
    BrowseTree,
    HistorySearch,
    HistoryReference,
    RebaseOnto,
    MergeRevision,
    AutosquashTarget {
        squash: bool,
    },
    AutosquashMessage {
        target: String,
    },
    RewordSubject,
    RewordBody {
        subject: String,
    },
    MergeToolPath,
    CreateBookmarkFolder,
    RenameBookmarkFolder {
        id: String,
    },
}

/// Apply Stash options dialog.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct StashApplyDialog {
    pub reference: String,
    pub delete_after: bool,
    pub restore_index: bool,
}

#[derive(Clone, Debug)]
pub(crate) enum ChoicePromptKind {
    SetMergeTool,
    MergePullRequest {
        number: u64,
    },
    ConfirmMergePullRequest {
        number: u64,
        method: git_domain::MergeMethod,
    },
    BookmarkFolderActions {
        id: String,
    },
    HistoryFilter,
    /// Soft / Mixed / Hard reset of HEAD to `oid`.
    ResetMode {
        oid: String,
    },
}

pub(crate) const MERGE_TOOL_CHOICES: &[&str] = &["opendiff", "meld", "kdiff3", "vimdiff", "bc3"];

pub(crate) const PR_MERGE_METHOD_CHOICES: &[(&str, git_domain::MergeMethod)] = &[
    ("Merge commit", git_domain::MergeMethod::Merge),
    ("Squash", git_domain::MergeMethod::Squash),
    ("Rebase", git_domain::MergeMethod::Rebase),
];

pub(crate) const HISTORY_FILTER_CHOICES: &[&str] = &[
    "Current branch",
    "All refs",
    "Branch or tag…",
    "Search history…",
    "Reveal HEAD",
    "Copy selected OID",
    "New branch from commit…",
];

pub(crate) const RESET_MODE_CHOICES: &[&str] = &["Soft", "Mixed (Keep Changes)", "Hard"];

impl ChoicePromptKind {
    pub(crate) fn title(&self) -> String {
        match self {
            Self::SetMergeTool => "Choose a merge tool".into(),
            Self::MergePullRequest { number } => {
                format!("Merge pull request #{number}")
            }
            Self::ConfirmMergePullRequest { number, method } => {
                let label = match method {
                    git_domain::MergeMethod::Merge => "Merge commit",
                    git_domain::MergeMethod::Squash => "Squash",
                    git_domain::MergeMethod::Rebase => "Rebase",
                };
                format!("Merge pull request #{number} using {label}?")
            }
            Self::BookmarkFolderActions { .. } => "Group".into(),
            Self::HistoryFilter => "History filter".into(),
            Self::ResetMode { oid } => {
                format!("Reset HEAD to \"{}\"", short_commit_label(oid))
            }
        }
    }

    pub(crate) fn options(&self) -> Vec<&'static str> {
        match self {
            Self::SetMergeTool => MERGE_TOOL_CHOICES.to_vec(),
            Self::MergePullRequest { .. } => PR_MERGE_METHOD_CHOICES
                .iter()
                .map(|(label, _)| *label)
                .collect(),
            Self::ConfirmMergePullRequest { .. } => Vec::new(),
            Self::BookmarkFolderActions { .. } => vec!["Rename…", "Delete Group"],
            Self::HistoryFilter => HISTORY_FILTER_CHOICES.to_vec(),
            Self::ResetMode { .. } => RESET_MODE_CHOICES.to_vec(),
        }
    }

    pub(crate) fn filtered_options(&self, query: &str) -> Vec<(usize, &'static str)> {
        let needle = query.trim().to_lowercase();
        self.options()
            .into_iter()
            .enumerate()
            .filter(|(_, label)| needle.is_empty() || label.to_lowercase().contains(&needle))
            .collect()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PaletteCommand {
    OpenRepository,
    Fetch,
    Pull,
    Push,
    Sync,
    RefreshWorkingCopy,
    ShowWorkingCopy,
    StageAll,
    UnstageAll,
    FocusCommitComposer,
    AmendLastCommit,
    SaveStash,
    SaveStashUntracked,
    SaveStashSnapshot,
    ApplyLatestStash,
    CreateBranch,
    CreateTag,
    ShowHistory,
    HistoryFilter,
    RevealHistoryHead,
    CopySelectedCommitHash,
    BranchFromSelectedCommit,
    CheckoutSelectedCommit,
    ResetHeadToSelected,
    RevertSelectedCommit,
    SaveSelectedPatch,
    ExportSelectedCommit,
    CompareSelectedCommit,
    CommitDetail,
    ShowStashes,
    ApplySelectedStash,
    ApplySelectedStashFiles,
    BranchFromSelectedStash,
    PopSelectedStash,
    DropSelectedStash,
    ShowRemotes,
    ShowSettings,
    ShowWorkflow,
    ShowPullRequests,
    ShowBranchesReview,
    GitLfsStatus,
    FetchGitLfs,
    PullGitLfs,
    ShowReflog,
    FileHistory,
    Blame,
    CompareRefs,
    BrowseTree,
    Worktrees,
    Submodules,
    RebasePlan,
    RebaseOnto,
    MergeRevision,
    SquashStaged,
    FixupStaged,
    DropCommit,
    RewordLastCommit,
    Conflicts,
    SetMergeTool,
    OpenInMergeTool,
    CheckCommitSignature,
    QuickOpenFile,
    ToggleMessageHistory,
    ToggleAppearance,
    ShowKeyboardShortcuts,
    NavigateBack,
    NavigateForward,
    AboutGitRonimo,
    CheckForUpdates,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum OverlayFocus {
    CommandPalette,
    TextPrompt,
    ChoicePrompt,
}

/// Pull dialog: remote branch + optional rebase.
#[derive(Clone, Debug)]
pub(crate) struct PullDialogState {
    pub use_rebase: bool,
    /// Selected remote-tracking ref (`origin/main`), or empty for configured upstream.
    pub remote_branch: String,
    pub remote_branches: Vec<String>,
    pub branch_menu_open: bool,
}

/// How `--recurse-submodules` behaves while pushing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SubmodulePushMode {
    /// Refuse the push when a referenced submodule commit is missing on its remote.
    Check,
    /// Push submodule commits that the superproject references.
    OnDemand,
}

impl SubmodulePushMode {
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Check => "Verify submodules before push",
            Self::OnDemand => "Push submodules on demand",
        }
    }

    pub(crate) const fn flag_value(self) -> &'static str {
        match self {
            Self::Check => "check",
            Self::OnDemand => "on-demand",
        }
    }

    pub(crate) const fn choices() -> [Self; 2] {
        [Self::Check, Self::OnDemand]
    }
}

/// A toggle in the Push dialog's Options list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PushOption {
    AllTags,
    Force,
    RecurseSubmodules,
    SkipHooks,
}

impl PushOption {
    pub(crate) const fn element_id(self) -> &'static str {
        match self {
            Self::AllTags => "push-all-tags",
            Self::Force => "push-force",
            Self::RecurseSubmodules => "push-recurse-submodules",
            Self::SkipHooks => "push-skip-hooks",
        }
    }

    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::AllTags => "Push All Tags",
            Self::Force => "Force Push",
            Self::RecurseSubmodules => "Recurse Submodules",
            Self::SkipHooks => "Skip Hooks",
        }
    }

    pub(crate) const fn caption(self) -> &'static str {
        match self {
            Self::AllTags => "All refs under refs/tags are pushed.",
            Self::Force => {
                "Enforces a new history on the remote branch when fast-forward is not possible. Uses a lease, so commits you have not seen are never discarded."
            }
            Self::RecurseSubmodules => {
                "Ensures that all submodule commits referenced by the superproject have been pushed."
            }
            Self::SkipHooks => "Bypasses any associated hook scripts.",
        }
    }
}

/// Push dialog: destination remote branch plus push options.
#[derive(Clone, Debug)]
pub(crate) struct PushDialogState {
    /// Local HEAD branch the commits come from.
    pub head_branch: String,
    /// Chosen remote-tracking ref (`origin/main`).
    pub destination: String,
    pub destinations: Vec<String>,
    pub destination_menu_open: bool,
    pub enabled_options: Vec<PushOption>,
    pub submodule_mode: SubmodulePushMode,
    pub submodule_menu_open: bool,
}

impl PushDialogState {
    pub(crate) fn is_enabled(&self, option: PushOption) -> bool {
        self.enabled_options.contains(&option)
    }

    pub(crate) fn toggle(&mut self, option: PushOption) {
        if let Some(index) = self
            .enabled_options
            .iter()
            .position(|enabled| *enabled == option)
        {
            self.enabled_options.remove(index);
        } else {
            self.enabled_options.push(option);
        }
    }
}

pub(crate) const PALETTE_COMMANDS: &[(&str, PaletteCommand)] = &[
    ("Open repository…", PaletteCommand::OpenRepository),
    ("Fetch", PaletteCommand::Fetch),
    ("Pull…", PaletteCommand::Pull),
    ("Push…", PaletteCommand::Push),
    ("Sync", PaletteCommand::Sync),
    ("Refresh working copy", PaletteCommand::RefreshWorkingCopy),
    ("Show working copy", PaletteCommand::ShowWorkingCopy),
    ("Stage all", PaletteCommand::StageAll),
    ("Unstage all", PaletteCommand::UnstageAll),
    ("Focus commit composer", PaletteCommand::FocusCommitComposer),
    ("Amend last commit", PaletteCommand::AmendLastCommit),
    ("Save stash…", PaletteCommand::SaveStash),
    (
        "Save stash including untracked…",
        PaletteCommand::SaveStashUntracked,
    ),
    ("Save stash snapshot…", PaletteCommand::SaveStashSnapshot),
    ("Apply latest stash…", PaletteCommand::ApplyLatestStash),
    ("Apply selected stash…", PaletteCommand::ApplySelectedStash),
    (
        "Apply selected stash files",
        PaletteCommand::ApplySelectedStashFiles,
    ),
    (
        "Branch from selected stash…",
        PaletteCommand::BranchFromSelectedStash,
    ),
    ("Pop selected stash…", PaletteCommand::PopSelectedStash),
    ("Drop selected stash…", PaletteCommand::DropSelectedStash),
    ("Create branch…", PaletteCommand::CreateBranch),
    ("Create tag…", PaletteCommand::CreateTag),
    ("Show history", PaletteCommand::ShowHistory),
    ("History filter…", PaletteCommand::HistoryFilter),
    ("Reveal HEAD in history", PaletteCommand::RevealHistoryHead),
    (
        "Copy selected commit hash",
        PaletteCommand::CopySelectedCommitHash,
    ),
    (
        "New branch from selected commit…",
        PaletteCommand::BranchFromSelectedCommit,
    ),
    (
        "Check out selected commit",
        PaletteCommand::CheckoutSelectedCommit,
    ),
    (
        "Reset HEAD to selected commit…",
        PaletteCommand::ResetHeadToSelected,
    ),
    (
        "Revert selected commit…",
        PaletteCommand::RevertSelectedCommit,
    ),
    (
        "Save patch for selected commit…",
        PaletteCommand::SaveSelectedPatch,
    ),
    (
        "Export selected commit…",
        PaletteCommand::ExportSelectedCommit,
    ),
    (
        "Compare selected commit…",
        PaletteCommand::CompareSelectedCommit,
    ),
    ("Commit detail…", PaletteCommand::CommitDetail),
    ("Show stashes", PaletteCommand::ShowStashes),
    ("Show remotes", PaletteCommand::ShowRemotes),
    ("Show settings", PaletteCommand::ShowSettings),
    ("Show workflow", PaletteCommand::ShowWorkflow),
    ("Show pull requests", PaletteCommand::ShowPullRequests),
    ("Branches review", PaletteCommand::ShowBranchesReview),
    ("Git LFS status", PaletteCommand::GitLfsStatus),
    ("Fetch Git LFS objects", PaletteCommand::FetchGitLfs),
    ("Pull Git LFS objects", PaletteCommand::PullGitLfs),
    ("Show reflog", PaletteCommand::ShowReflog),
    ("File history…", PaletteCommand::FileHistory),
    ("Blame…", PaletteCommand::Blame),
    ("Compare refs…", PaletteCommand::CompareRefs),
    ("Browse tree at commit…", PaletteCommand::BrowseTree),
    ("Worktrees…", PaletteCommand::Worktrees),
    ("Submodules…", PaletteCommand::Submodules),
    ("Rebase plan…", PaletteCommand::RebasePlan),
    ("Rebase onto…", PaletteCommand::RebaseOnto),
    ("Merge revision…", PaletteCommand::MergeRevision),
    ("Squash staged changes…", PaletteCommand::SquashStaged),
    ("Fixup staged changes…", PaletteCommand::FixupStaged),
    ("Drop commit…", PaletteCommand::DropCommit),
    ("Reword last commit…", PaletteCommand::RewordLastCommit),
    ("Conflicts…", PaletteCommand::Conflicts),
    ("Set merge tool…", PaletteCommand::SetMergeTool),
    ("Open in merge tool…", PaletteCommand::OpenInMergeTool),
    (
        "Check commit signature…",
        PaletteCommand::CheckCommitSignature,
    ),
    ("Quick open file…", PaletteCommand::QuickOpenFile),
    ("Message history", PaletteCommand::ToggleMessageHistory),
    ("Toggle appearance", PaletteCommand::ToggleAppearance),
    (
        "Show keyboard shortcuts",
        PaletteCommand::ShowKeyboardShortcuts,
    ),
    ("Navigate back", PaletteCommand::NavigateBack),
    ("Navigate forward", PaletteCommand::NavigateForward),
    ("About GitRonimo", PaletteCommand::AboutGitRonimo),
    ("Check for updates", PaletteCommand::CheckForUpdates),
];

impl PaletteCommand {
    pub(crate) fn filtered(query: &str) -> Vec<(usize, &'static str, PaletteCommand)> {
        let needle = query.trim().to_lowercase();
        PALETTE_COMMANDS
            .iter()
            .enumerate()
            .filter(|(_, (label, _))| needle.is_empty() || label.to_lowercase().contains(&needle))
            .map(|(index, (label, command))| (index, *label, *command))
            .collect()
    }
}

#[cfg(test)]
mod palette_tests {
    use super::{PALETTE_COMMANDS, PaletteCommand};

    #[test]
    fn palette_includes_toolbar_and_shell_commands() {
        let labels: Vec<&str> = PALETTE_COMMANDS.iter().map(|(label, _)| *label).collect();
        for expected in [
            "Fetch",
            "Pull…",
            "Push…",
            "Sync",
            "Stage all",
            "Save stash…",
            "Save stash snapshot…",
            "Apply selected stash…",
            "Apply selected stash files",
            "Create branch…",
            "Create tag…",
            "Amend last commit",
            "History filter…",
            "Reset HEAD to selected commit…",
            "Show settings",
            "Show workflow",
            "Quick open file…",
            "Message history",
            "About GitRonimo",
            "Check for updates",
            "Fetch Git LFS objects",
            "Pull Git LFS objects",
        ] {
            assert!(
                labels.contains(&expected),
                "missing palette command: {expected}"
            );
        }
        assert!(
            PaletteCommand::filtered("push")
                .iter()
                .any(|(_, label, _)| { *label == "Push…" })
        );
        assert!(
            PaletteCommand::filtered("").len() >= 55,
            "expected expanded palette, got {}",
            PaletteCommand::filtered("").len()
        );
    }
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

/// In-window drag payload for applying selected stash paths onto Working Copy.
#[derive(Clone)]
pub(crate) struct StashPathDrag {
    pub reference: String,
    pub paths: Vec<GitPath>,
    pub label: String,
    pub colors: ThemeColors,
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
    /// Hovering a pane resize handle; col-resize cursor lives on a non-drag ancestor.
    pub resize_handle_hovered: bool,
    pub state: ShellState,
    pub recents: Vec<PathBuf>,
    pub selected_recent: Option<usize>,
    pub welcome_snapshot: Option<WelcomeRepoSnapshot>,
    pub welcome_snapshot_path: Option<PathBuf>,
    pub welcome_snapshot_token: u64,
    pub welcome_list_snapshots: std::collections::HashMap<PathBuf, WelcomeRepoSnapshot>,
    pub welcome_list_snapshot_token: u64,
    pub welcome_shell_view: WelcomeShellView,
    pub bookmark_folders: Vec<app_core::BookmarkFolder>,
    pub repository_folders: std::collections::HashMap<PathBuf, String>,
    pub welcome_repo_search: String,
    pub worktree_file_search: String,
    #[allow(dead_code)]
    pub search_focus_handle: FocusHandle,
    pub commit_subject_focused: bool,
    pub commit_body_focused: bool,
    /// Expandable commit card: details (body/options/author) shown when expanded.
    pub commit_composer_expanded: bool,
    pub network_progress: f32,
    pub last_network_result: Option<String>,
    pub activity: String,
    /// Newest-first ring of recent activity messages (cap [`ACTIVITY_LOG_CAPACITY`]).
    pub activity_log: VecDeque<ActivityLogEntry>,
    pub show_activity_log: bool,
    pub working_copy: Option<git_domain::WorktreeStatus>,
    pub worktree_show_all_files: bool,
    pub tracked_files: Vec<git_domain::GitPath>,
    pub refs: RefSnapshot,
    pub expanded_ref_groups: BTreeSet<String>,
    pub ref_context: Option<RefContext>,
    /// Window coordinates of the right-click that opened the ref menu.
    pub ref_context_menu_position: Option<(f32, f32)>,
    pub ref_context_submenu: Option<RefContextSubmenu>,
    /// History commit under the right-click context menu.
    pub commit_context: Option<CommitContext>,
    pub commit_context_menu_position: Option<(f32, f32)>,
    /// Pinned and archived local branches for the open repository.
    pub branch_organization: app_core::BranchOrganization,
    /// Branching convention for the open repository or selected welcome recent.
    pub workflow: Option<app_core::RepositoryWorkflow>,
    pub selected_paths: Vec<GitPath>,
    pub last_selected_path_index: Option<usize>,
    /// Row that cleared a full selection; clicking it again re-selects every visible file.
    /// Clicking any other row selects that row alone.
    pub file_list_select_all_toggle: Option<GitPath>,
    /// Mouse-down origin for dragging a Working Copy path to another macOS app.
    pub file_drag_origin: Option<(GitPath, f32, f32)>,
    pub context_path: Option<GitPath>,
    pub loaded_diff: Option<LoadedDiff>,
    pub selected_diff: Option<(GitPath, bool)>,
    pub selected_diff_lines: Vec<(usize, usize)>,
    pub pending_line_discard: Option<(GitPath, Vec<(usize, usize)>)>,
    pub pending_hunk_discard: Option<(GitPath, usize)>,
    pub pending_discard: Option<Vec<GitPath>>,
    pub pending_operation_action: Option<OperationAction>,
    pub pending_branch_delete: Option<String>,
    pub confirm_dialog: Option<AppConfirmDialog>,
    pub pending_text_prompt: Option<TextPromptKind>,
    pub text_prompt_value: String,
    pub pending_choice_prompt: Option<ChoicePromptKind>,
    pub choice_prompt_query: String,
    pub choice_prompt_selected: usize,
    pub pull_dialog: Option<PullDialogState>,
    pub push_dialog: Option<PushDialogState>,
    pub stash_apply_dialog: Option<StashApplyDialog>,
    pub show_command_palette: bool,
    pub command_palette_query: String,
    pub command_palette_selected: usize,
    pub show_about: bool,
    pub pending_overlay_focus: Option<OverlayFocus>,
    pub selected_branch_review: Option<String>,
    pub branches_review_show_all: bool,
    pub force_push_state: ForcePushState,
    pub shortcut_reference_state: ShortcutReferenceState,
    pub commit_subject: String,
    pub commit_body: String,
    pub commit_amend: bool,
    /// Short oid of HEAD while amend is armed (shown beside the checkbox).
    pub commit_amend_short_oid: Option<String>,
    /// Subject/body restored when amend is turned off.
    pub commit_pre_amend_draft: Option<(String, String)>,
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
    pub stash_selection_token: u64,
    pub selected_stash_paths: Vec<GitPath>,
    pub stash_apply_selection: Vec<GitPath>,
    pub selected_stash_diff: Option<LoadedDiff>,
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
    pub github_load_token: u64,
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
    /// Settings override: skip `gix` and spawn the installed Git executable.
    pub use_system_git: bool,
    /// Settings: stash dirty work before switch and pull, then reapply it.
    pub auto_stash: bool,
    /// Settings: check GitHub Releases and replace this `.app` (off by default).
    pub in_app_updates: bool,
    pub pending_app_update: Option<crate::app_update::PendingAppUpdate>,
    pub diagnostics: String,
    pub subscriptions: Vec<gpui::Subscription>,
    pub column_width: f32,
    pub welcome_search_input: gpui::Entity<crate::views::single_line_input::SingleLineInput>,
    pub worktree_search_input: gpui::Entity<crate::views::single_line_input::SingleLineInput>,
    pub commit_subject_input: gpui::Entity<crate::views::single_line_input::SingleLineInput>,
    pub commit_body_input: gpui::Entity<crate::views::single_line_input::SingleLineInput>,
    pub text_prompt_input: gpui::Entity<crate::views::single_line_input::SingleLineInput>,
    pub command_palette_input: gpui::Entity<crate::views::single_line_input::SingleLineInput>,
    pub choice_prompt_input: gpui::Entity<crate::views::single_line_input::SingleLineInput>,
    pub show_quick_open: bool,
    /// Anchored menu from the welcome sidebar footer `+` button.
    pub welcome_plus_menu_open: bool,
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
    let error = redact_git_text(error);
    if error.to_lowercase().contains("index.lock") {
        format!(
            "{label} could not run because Git's index is locked. Check that no Git process is still running; if none is, inspect .git/index.lock before removing it manually."
        )
    } else {
        format!("{label} failed: {error}")
    }
}

/// Git refuses `git branch --delete` when the tip is not reachable from HEAD.
pub(crate) fn branch_not_fully_merged_error(error: &str) -> bool {
    let lower = error.to_lowercase();
    lower.contains("not fully merged") || lower.contains("isn't fully merged")
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

pub(crate) fn clamp_sidebar_width(width: f32) -> f32 {
    width.clamp(MINIMUM_PANE_WIDTH, MAXIMUM_PANE_WIDTH)
}

pub(crate) fn clamp_list_pane_width(width: f32) -> f32 {
    width.clamp(MINIMUM_LIST_PANE_WIDTH, MAXIMUM_LIST_PANE_WIDTH)
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

/// Absolute worktree path that is safe to hand to another macOS app as a file URL.
#[must_use]
pub(crate) fn eligible_external_drag_path(root: &Path, path: &GitPath) -> Option<PathBuf> {
    use std::{ffi::OsString, os::unix::ffi::OsStringExt};

    if path.0.starts_with(b"/")
        || path.0.split(|byte| *byte == b'/').any(|part| part == b"..")
        || std::str::from_utf8(&path.0).is_err()
    {
        return None;
    }
    let target = root.join(OsString::from_vec(path.0.clone()));
    target.symlink_metadata().ok()?;
    Some(target)
}

/// Files to put on an `AppKit` drag: the row alone, or every selected existing path
/// when that row is part of a multi-selection.
#[must_use]
pub(crate) fn files_for_status_drag(
    root: &Path,
    row: &GitPath,
    selected: &[GitPath],
) -> Vec<PathBuf> {
    let paths: &[GitPath] = if selected.iter().any(|path| path == row) && selected.len() > 1 {
        selected
    } else {
        std::slice::from_ref(row)
    };
    paths
        .iter()
        .filter_map(|path| eligible_external_drag_path(root, path))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{
        ActivityKind, AppConfirmDialog, branch_not_fully_merged_error, classify_activity,
        is_working_copy_refresh_noise,
    };

    #[test]
    fn classifies_success_error_and_confirm_activity() {
        assert_eq!(
            classify_activity("Pushing main to origin/main complete."),
            ActivityKind::Success
        );
        assert_eq!(
            classify_activity("Could not delete \"feature\": it contains unmerged changes."),
            ActivityKind::Error
        );
        assert_eq!(
            classify_activity("Confirm deletion of branch feature/ui-improvements."),
            ActivityKind::Confirm
        );
        assert_eq!(
            classify_activity("Refreshing working copy…"),
            ActivityKind::Progress
        );
        assert!(is_working_copy_refresh_noise(
            "Working copy refreshed: 0 change(s)."
        ));
        assert!(!is_working_copy_refresh_noise(
            "Pushing main to origin/main complete."
        ));
    }

    #[test]
    fn detects_not_fully_merged_git_errors() {
        assert!(branch_not_fully_merged_error(
            "error: The branch 'feature/ui-improvements' is not fully merged."
        ));
        assert!(branch_not_fully_merged_error(
            "GitError(\"The branch 'x' isn't fully merged.\\n\")"
        ));
        assert!(!branch_not_fully_merged_error(
            "error: Cannot delete branch 'main' checked out at ..."
        ));
    }

    #[test]
    fn force_delete_dialog_copy() {
        let dialog = AppConfirmDialog::ForceDeleteBranch {
            branch: "feature/ui-improvements".into(),
        };
        assert_eq!(dialog.title(), "Could Not Delete Branch");
        assert!(dialog.body().contains("feature/ui-improvements"));
        assert!(dialog.body().contains("unmerged changes"));
        assert_eq!(AppConfirmDialog::cancel_label(), "Cancel");
        assert_eq!(dialog.confirm_label(), "Delete");
    }

    #[test]
    fn finish_topic_dialog_copy() {
        let dialog = AppConfirmDialog::FinishTopic {
            branch: "feature/login".into(),
            label: "Feature".into(),
            targets: "main".into(),
            strategy: "Merge".into(),
        };
        assert_eq!(dialog.title(), "Finish Topic Branch");
        assert!(dialog.body().contains("feature/login"));
        assert!(dialog.body().contains("main"));
        assert_eq!(dialog.confirm_label(), "Finish");
    }

    #[test]
    fn install_update_dialog_copy() {
        let dialog = AppConfirmDialog::InstallUpdate {
            version: "1.0.1".into(),
        };
        assert_eq!(dialog.title(), "Install Update");
        assert!(dialog.body().contains("1.0.1"));
        assert!(dialog.body().contains("SHA-256"));
        assert!(dialog.body().contains("Gatekeeper"));
        assert_eq!(dialog.confirm_label(), "Install");
    }
}
