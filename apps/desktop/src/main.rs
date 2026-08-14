//! macOS application entry point and state-mutating command methods.
//!
//! Window layout lives in `views/`; shared shell types live in `app_state`.
//! Keeping all state transitions here (never in render modules) preserves the
//! rule that Git and domain logic do not appear in GPUI render implementations.

mod actions;
mod app_state;
mod assets;
mod keymap;
mod menus;
#[cfg(test)]
mod tests;
mod views;

use std::{
    ffi::OsString,
    fs,
    io::{BufRead, BufReader},
    os::unix::ffi::OsStringExt,
    path::{Path, PathBuf},
    process::Command,
    sync::{Arc, Mutex, mpsc},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use app_core::{
    BookmarkFolder, BookmarkOrganization, BranchOrganization, HostingError, HostingService,
    RecentRepositoryStore, RecoveryJournalStore, RepositoryOpenError, RepositoryWorkflow,
    SecretStore, WindowGeometry, WorkflowGitStep, WorkflowKind, open_repository,
};
use git_cli::{
    CommitRequest, CreateStashRequest, GitExecutable, GitStatusError, ResetMode,
    parse_git_progress_line,
};
use git_domain::{
    ConflictSide, FileHistoryRequest, GitPath, GraphState, HeadStatus, HistoryPage,
    HistoryReference, HistoryRequest, RefSnapshot, ReflogRequest, TreeEntry, TreeEntryKind,
    WorktreeRepository, layout_history_graph,
};
use gpui::{
    App, Application, Bounds, ClipboardItem, Context, ExternalPaths, Focusable, ListAlignment,
    ListState, PathPromptOptions, Window, WindowBounds, WindowOptions, point, prelude::*, px, size,
};
use hosting_github::GitHubService;
use notify::{RecursiveMode, Watcher};
use platform_macos::MacKeychainStore;
use ui_kit::Appearance;

use crate::actions::{
    About, CommandPalette, FocusComposer, FocusSearch, Hide, HistoryNext, HistoryPrevious,
    NavigateBack, NavigateForward, OpenRepository, Quit, Refresh, SaveStash, SelectAllStatusFiles,
    ShortcutReference, ToggleAppearance, WidenSidebar,
};
use crate::app_state::{
    ACTIVITY_LOG_CAPACITY, ActivityLogEntry, AppConfirmDialog, ChoicePromptKind, CommitContext,
    DEFAULT_LIST_PANE_WIDTH, DEFAULT_SIDEBAR_WIDTH, ForcePushState, GitronimoApp,
    HistoryDetailMode, LastAction, Mutation, NetworkOperation, OpenedRepository, OperationAction,
    OverlayFocus, PR_MERGE_METHOD_CHOICES, PaletteCommand, PullDialogState, PushDialogState,
    PushOption, RefContext, RefContextSubmenu, RepositoryView, ShellState, ShortcutReferenceState,
    StashAction, StashApplyDialog, SubmodulePushMode, TextPromptKind, ThemeMode,
    WelcomeRepoSnapshot, WelcomeShellView, appearance_from_window, branch_not_fully_merged_error,
    clamp_list_pane_width, clamp_sidebar_width, classify_activity, discard_selected,
    git_failure_message, is_working_copy_refresh_noise, network_failure_message,
    repository_is_available, repository_unavailable_message, resize_width,
};
use crate::views::components::status_path;
use crate::views::single_line_input::register_input_bindings;
use crate::views::working_copy::entry_is_staged;

const INITIAL_WINDOW_SIZE: (f32, f32) = (1200.0, 800.0);
const MINIMUM_WINDOW_SIZE: (f32, f32) = (800.0, 560.0);
const CREATE_PULL_REQUEST_SCRIPT: &str = r#"set title_text to text returned of (display dialog "Pull request title" default answer "")
set body_text to text returned of (display dialog "Pull request description" default answer "")
set head_text to text returned of (display dialog "Head branch" default answer "")
set base_text to text returned of (display dialog "Base branch" default answer "main")
return title_text & linefeed & body_text & linefeed & head_text & linefeed & base_text"#;
fn main() {
    install_panic_reporter();
    Application::new()
        .with_assets(assets::DesktopAssets)
        .run(|cx: &mut App| {
            cx.bind_keys(keymap::bindings());
            cx.set_menus(menus::application_menus());
            cx.on_action(|_: &Quit, cx: &mut App| cx.quit());
            cx.on_action(|_: &Hide, cx: &mut App| cx.hide());
            register_input_bindings(cx);

            let store = RecentRepositoryStore::new(preferences_path());
            let _ = store.recover_corrupted_preferences();
            let recents = store.load().unwrap_or_default();
            let geometry = store.load_window_geometry().ok().flatten();
            install_folder_picker(cx, store.clone());
            if let Err(error) = cx.open_window(window_options(cx, geometry), |window, cx| {
                let app = cx.new(|cx| GitronimoApp::welcome(recents, store, window, cx));
                window.focus(&app.read(cx).focus_handle);
                app
            }) {
                eprintln!("Unable to open the Gitronimo window: {error}");
                return;
            }
            cx.activate(true);
        });
}

fn install_folder_picker(cx: &mut App, store: RecentRepositoryStore) {
    cx.on_action(move |_: &OpenRepository, cx| {
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Choose a Git repository".into()),
        });
        let store = store.clone();
        cx.spawn(async move |cx| {
            let Ok(Ok(Some(paths))) = receiver.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            let outcome = cx
                .background_spawn(async move { discover_and_record(&path, &store) })
                .await;
            let _ = cx.update(|cx| open_result_window(cx, outcome));
        })
        .detach();
    });
}

fn open_result_window(cx: &mut App, outcome: Result<OpenedRepository, RepositoryOpenError>) {
    let _ = cx.open_window(window_options(cx, None), |window, cx| {
        let app = cx.new(|cx| GitronimoApp::from_open_outcome(outcome, window, cx));
        window.focus(&app.read(cx).focus_handle);
        app
    });
}

fn discover_and_record(
    path: &Path,
    store: &RecentRepositoryStore,
) -> Result<OpenedRepository, RepositoryOpenError> {
    let _ = store.recover_corrupted_preferences();
    let git = GitExecutable::discover().map_err(|_| RepositoryOpenError::DiscoveryFailed)?;
    let repository = open_repository(&git, path)?;
    let recents = store
        .record(repository.worktree_root.clone())
        .unwrap_or_default();
    Ok(OpenedRepository {
        repository,
        recents,
    })
}

fn load_welcome_snapshot(path: &Path) -> WelcomeRepoSnapshot {
    let mut snapshot = WelcomeRepoSnapshot {
        last_modified: fs::metadata(path)
            .ok()
            .and_then(|meta| meta.modified().ok()),
        ..WelcomeRepoSnapshot::default()
    };
    if !path.is_dir() {
        return snapshot;
    }
    let Ok(git) = GitExecutable::discover() else {
        return snapshot;
    };
    let Ok(repository) = open_repository(&git, path) else {
        return snapshot;
    };
    snapshot.available = true;
    if let Ok(status) = git.worktree_status(&repository, false) {
        snapshot.branch = match status.branch.head {
            HeadStatus::Branch(name) => Some(String::from_utf8_lossy(&name.0).into_owned()),
            HeadStatus::Detached => Some("Detached HEAD".into()),
            HeadStatus::Unborn => Some("Unborn branch".into()),
            HeadStatus::Unknown => None,
        };
        snapshot.upstream = status
            .branch
            .upstream
            .as_ref()
            .map(|upstream| String::from_utf8_lossy(&upstream.0).into_owned());
        snapshot.ahead = status.branch.ahead;
        snapshot.behind = status.branch.behind;
        snapshot.changed_files = Some(status.entries.len());
    }
    if let Ok(refs) = git.ref_snapshot(&repository) {
        snapshot.remote_url = refs
            .remotes
            .iter()
            .find(|remote| remote.name.0 == b"origin")
            .or_else(|| refs.remotes.first())
            .map(|remote| String::from_utf8_lossy(&remote.fetch_url).into_owned());
    }
    if let Ok(Some(identity)) = git.author_identity(&repository) {
        snapshot.author_name = Some(identity.name);
        snapshot.author_email = Some(identity.email);
    }
    if let Ok(page) = git.history_page(
        &repository,
        &HistoryRequest {
            reference: HistoryReference::Current,
            before: None,
            limit: 1,
        },
    ) && let Some(commit) = page.commits.first()
    {
        snapshot.last_commit_subject = Some(String::from_utf8_lossy(&commit.subject).into_owned());
    }
    snapshot
}

fn preferences_path() -> PathBuf {
    std::env::var_os("HOME")
        .map_or_else(std::env::temp_dir, PathBuf::from)
        .join("Library/Application Support/Gitronimo/recent-repositories.json")
}

fn recovery_journal_path() -> PathBuf {
    preferences_path()
        .parent()
        .map_or_else(std::env::temp_dir, Path::to_path_buf)
        .join("recovery-journal.json")
}

fn joined_tree_path(segments: &[GitPath]) -> GitPath {
    let mut bytes = Vec::new();
    for segment in segments {
        if !bytes.is_empty() {
            bytes.push(b'/');
        }
        bytes.extend_from_slice(&segment.0);
    }
    GitPath(bytes)
}

fn service_auth_state(error: &HostingError) -> git_domain::ServiceAuthState {
    match error {
        HostingError::Authentication => git_domain::ServiceAuthState::Expired,
        HostingError::RateLimited { .. } => git_domain::ServiceAuthState::RateLimited,
        HostingError::Network | HostingError::Api(_) | HostingError::Parse => {
            git_domain::ServiceAuthState::Error("GitHub could not be loaded.".into())
        }
    }
}

fn install_panic_reporter() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let directory = preferences_path()
            .parent()
            .map_or_else(std::env::temp_dir, Path::to_path_buf)
            .join("crash-reports");
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.as_millis());
        let _ = fs::create_dir_all(&directory);
        let _ = fs::write(
            crash_report_path(&directory, timestamp),
            crash_report_body(timestamp, info.location()),
        );
        previous(info);
    }));
}

fn crash_report_path(directory: &Path, timestamp: u128) -> PathBuf {
    directory.join(format!("gitronimo-crash-{timestamp}.txt"))
}

fn crash_report_body(timestamp: u128, location: Option<&std::panic::Location<'_>>) -> String {
    let location = location.map_or_else(
        || "unknown location".to_owned(),
        |location| format!("{}:{}", location.file(), location.line()),
    );
    format!(
        "Gitronimo stopped unexpectedly.\nTimestamp: {timestamp}\nLocation: {location}\n\nThis report stays on this Mac and is never uploaded automatically.\n"
    )
}

fn window_options(cx: &App, geometry: Option<WindowGeometry>) -> WindowOptions {
    let initial_size = size(px(INITIAL_WINDOW_SIZE.0), px(INITIAL_WINDOW_SIZE.1));
    let window_bounds = geometry
        .filter(|geometry| {
            geometry.width >= MINIMUM_WINDOW_SIZE.0 && geometry.height >= MINIMUM_WINDOW_SIZE.1
        })
        .map_or_else(
            || WindowBounds::Windowed(Bounds::centered(None, initial_size, cx)),
            |geometry| {
                WindowBounds::Windowed(Bounds::new(
                    point(px(geometry.x), px(geometry.y)),
                    size(px(geometry.width), px(geometry.height)),
                ))
            },
        );
    WindowOptions {
        window_bounds: Some(window_bounds),
        titlebar: Some(gpui::TitlebarOptions {
            title: Some("Gitronimo".into()),
            ..Default::default()
        }),
        window_min_size: Some(size(px(MINIMUM_WINDOW_SIZE.0), px(MINIMUM_WINDOW_SIZE.1))),
        ..Default::default()
    }
}

impl GitronimoApp {
    #[allow(clippy::too_many_lines)]
    fn welcome(
        recents: Vec<PathBuf>,
        store: RecentRepositoryStore,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let expanded_ref_groups = store
            .load_expanded_ref_groups()
            .unwrap_or_default()
            .into_iter()
            .collect();
        let organization = store.load_bookmark_organization().unwrap_or_default();
        let bookmark_folders = organization.folders;
        let repository_folders = organization
            .repository_folders
            .into_iter()
            .map(|(path, folder)| (PathBuf::from(path), folder))
            .collect();
        let sidebar_width = store
            .load_sidebar_width()
            .ok()
            .flatten()
            .map_or(DEFAULT_SIDEBAR_WIDTH, clamp_sidebar_width);
        let column_width = store
            .load_list_pane_width()
            .ok()
            .flatten()
            .map_or(DEFAULT_LIST_PANE_WIDTH, clamp_list_pane_width);
        let branch_organization = BranchOrganization::default();
        let selected_recent = (!recents.is_empty()).then_some(0);
        let workflow = selected_recent
            .and_then(|index| recents.get(index))
            .and_then(|path| store.load_workflow(path).ok().flatten());
        let (
            welcome_search_input,
            worktree_search_input,
            commit_subject_input,
            commit_body_input,
            text_prompt_input,
            command_palette_input,
            choice_prompt_input,
        ) = Self::create_text_inputs(cx);
        let mut app = Self {
            focus_handle: cx.focus_handle(),
            last_action: None,
            appearance: appearance_from_window(window.appearance()),
            theme_mode: ThemeMode::System,
            sidebar_width,
            resize_handle_hovered: false,
            state: ShellState::Welcome,
            recents,
            selected_recent,
            welcome_snapshot: None,
            welcome_snapshot_path: None,
            welcome_snapshot_token: 0,
            welcome_list_snapshots: std::collections::HashMap::new(),
            welcome_list_snapshot_token: 0,
            welcome_shell_view: WelcomeShellView::Repositories,
            bookmark_folders,
            repository_folders,
            welcome_repo_search: String::new(),
            worktree_file_search: String::new(),
            search_focus_handle: cx.focus_handle(),
            commit_subject_focused: false,
            commit_body_focused: false,
            commit_composer_expanded: false,
            network_progress: 0.0,
            last_network_result: None,
            activity: "Choose a repository to begin.".into(),
            activity_log: std::collections::VecDeque::from([ActivityLogEntry {
                message: "Choose a repository to begin.".into(),
                kind: classify_activity("Choose a repository to begin."),
                at: SystemTime::now(),
            }]),
            show_activity_log: false,
            working_copy: None,
            worktree_show_all_files: false,
            tracked_files: Vec::new(),
            refs: RefSnapshot::default(),
            expanded_ref_groups,
            ref_context: None,
            ref_context_menu_position: None,
            ref_context_submenu: None,
            commit_context: None,
            commit_context_menu_position: None,
            branch_organization,
            workflow,
            selected_paths: Vec::new(),
            last_selected_path_index: None,
            file_list_select_all_toggle: None,
            context_path: None,
            loaded_diff: None,
            selected_diff: None,
            selected_diff_lines: Vec::new(),
            pending_line_discard: None,
            pending_hunk_discard: None,
            pending_discard: None,
            pending_operation_action: None,
            pending_branch_delete: None,
            confirm_dialog: None,
            pending_text_prompt: None,
            text_prompt_value: String::new(),
            pending_choice_prompt: None,
            choice_prompt_query: String::new(),
            choice_prompt_selected: 0,
            pull_dialog: None,
            push_dialog: None,
            stash_apply_dialog: None,
            show_command_palette: false,
            command_palette_query: String::new(),
            command_palette_selected: 0,
            show_about: false,
            pending_overlay_focus: None,
            selected_branch_review: None,
            branches_review_show_all: false,
            force_push_state: ForcePushState::Idle,
            shortcut_reference_state: ShortcutReferenceState::Hidden,
            commit_subject: String::new(),
            commit_body: String::new(),
            commit_amend: false,
            commit_amend_short_oid: None,
            commit_pre_amend_draft: None,
            commit_sign_off: false,
            author_identity: "Loading author identity…".into(),
            repository_view: RepositoryView::WorkingCopy,
            navigation_back: Vec::new(),
            navigation_forward: Vec::new(),
            came_from_welcome: false,
            history: Vec::new(),
            history_rows: Vec::new(),
            history_state: GraphState::default(),
            history_reference: HistoryReference::All,
            history_next: None,
            history_decorations: Vec::new(),
            selected_history: None,
            history_search: String::new(),
            history_list_state: ListState::new(0, ListAlignment::Top, px(72.0)),
            history_paths: Vec::new(),
            history_diff: None,
            history_selection_token: 0,
            history_load_token: 0,
            history_reveal_oid: None,
            history_detail_mode: HistoryDetailMode::Changeset,
            stashes: Vec::new(),
            stashes_load_token: 0,
            selected_stash: None,
            stash_selection_token: 0,
            selected_stash_paths: Vec::new(),
            selected_stash_diff: None,
            pending_stash_action_ref: None,
            reflog: Vec::new(),
            reflog_load_token: 0,
            selected_reflog: None,
            file_history: Vec::new(),
            file_history_path: String::new(),
            file_history_load_token: 0,
            blame: Vec::new(),
            blame_path: String::new(),
            blame_load_token: 0,
            compare_diff: None,
            compare_left: String::new(),
            compare_right: String::new(),
            compare_load_token: 0,
            tree: Vec::new(),
            tree_oid: String::new(),
            tree_path: Vec::new(),
            tree_blob: None,
            tree_blob_path: None,
            tree_load_token: 0,
            worktrees: Vec::new(),
            worktrees_load_token: 0,
            submodules: Vec::new(),
            submodules_load_token: 0,
            lfs: Vec::new(),
            lfs_load_token: 0,
            service_auth_state: git_domain::ServiceAuthState::SignedOut,
            service_account: None,
            hosted_repositories: Vec::new(),
            github_load_token: 0,
            pull_requests: Vec::new(),
            selected_pull_request: None,
            pull_request_detail: None,
            pull_request_repository: None,
            pull_requests_load_token: 0,
            pull_request_detail_token: 0,
            rebase_plan: Vec::new(),
            rebase_plan_load_token: 0,
            conflict_path: None,
            conflict_content: None,
            mutation_in_flight: false,
            network_operation: None,
            watcher: None,
            watch_events: None,
            store,
            diagnostics: "Checking Git installation…".into(),
            subscriptions: Vec::new(),
            column_width,
            welcome_search_input,
            worktree_search_input,
            commit_subject_input,
            commit_body_input,
            text_prompt_input,
            command_palette_input,
            choice_prompt_input,
            show_quick_open: false,
            welcome_plus_menu_open: false,
            last_commit_summary: None,
            file_diff_stats: std::collections::HashMap::new(),
        };
        app.observe_system_appearance(window, cx);
        app.observe_window_geometry(window, cx);
        app.observe_commit_composer_focus(window, cx);
        Self::load_diagnostics(cx);
        if app.selected_recent.is_some() {
            app.refresh_welcome_snapshot(cx);
        }
        app.refresh_welcome_list_snapshots(cx);
        app
    }

    fn from_open_outcome(
        outcome: Result<OpenedRepository, RepositoryOpenError>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        match outcome {
            Ok(opened) => Self::new_shell(
                ShellState::Repository(opened.repository),
                opened.recents,
                "Repository opened.".into(),
                RecentRepositoryStore::new(preferences_path()),
                window,
                cx,
            ),
            Err(error) => Self::new_shell(
                ShellState::Error(error.to_string()),
                Vec::new(),
                "Repository could not be opened.".into(),
                RecentRepositoryStore::new(preferences_path()),
                window,
                cx,
            ),
        }
    }

    #[allow(clippy::too_many_lines)]
    fn new_shell(
        state: ShellState,
        recents: Vec<PathBuf>,
        activity: String,
        store: RecentRepositoryStore,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let expanded_ref_groups = store
            .load_expanded_ref_groups()
            .unwrap_or_default()
            .into_iter()
            .collect();
        let organization = store.load_bookmark_organization().unwrap_or_default();
        let bookmark_folders = organization.folders;
        let repository_folders = organization
            .repository_folders
            .into_iter()
            .map(|(path, folder)| (PathBuf::from(path), folder))
            .collect();
        let sidebar_width = store
            .load_sidebar_width()
            .ok()
            .flatten()
            .map_or(DEFAULT_SIDEBAR_WIDTH, clamp_sidebar_width);
        let column_width = store
            .load_list_pane_width()
            .ok()
            .flatten()
            .map_or(DEFAULT_LIST_PANE_WIDTH, clamp_list_pane_width);
        let branch_organization = match &state {
            ShellState::Repository(repository) => store
                .load_branch_organization(&repository.worktree_root)
                .unwrap_or_default(),
            _ => BranchOrganization::default(),
        };
        let workflow = match &state {
            ShellState::Repository(repository) => store
                .load_workflow(&repository.worktree_root)
                .ok()
                .flatten(),
            _ => None,
        };
        let (
            welcome_search_input,
            worktree_search_input,
            commit_subject_input,
            commit_body_input,
            text_prompt_input,
            command_palette_input,
            choice_prompt_input,
        ) = Self::create_text_inputs(cx);
        let mut app = Self {
            focus_handle: cx.focus_handle(),
            last_action: None,
            appearance: appearance_from_window(window.appearance()),
            theme_mode: ThemeMode::System,
            sidebar_width,
            resize_handle_hovered: false,
            state,
            recents,
            selected_recent: None,
            welcome_snapshot: None,
            welcome_snapshot_path: None,
            welcome_snapshot_token: 0,
            welcome_list_snapshots: std::collections::HashMap::new(),
            welcome_list_snapshot_token: 0,
            welcome_shell_view: WelcomeShellView::Repositories,
            bookmark_folders,
            repository_folders,
            welcome_repo_search: String::new(),
            worktree_file_search: String::new(),
            search_focus_handle: cx.focus_handle(),
            commit_subject_focused: false,
            commit_body_focused: false,
            commit_composer_expanded: false,
            network_progress: 0.0,
            last_network_result: None,
            activity_log: std::collections::VecDeque::from([ActivityLogEntry {
                message: activity.clone(),
                kind: classify_activity(&activity),
                at: SystemTime::now(),
            }]),
            activity,
            show_activity_log: false,
            working_copy: None,
            worktree_show_all_files: false,
            tracked_files: Vec::new(),
            refs: RefSnapshot::default(),
            expanded_ref_groups,
            ref_context: None,
            ref_context_menu_position: None,
            ref_context_submenu: None,
            commit_context: None,
            commit_context_menu_position: None,
            branch_organization,
            workflow,
            selected_paths: Vec::new(),
            last_selected_path_index: None,
            file_list_select_all_toggle: None,
            context_path: None,
            loaded_diff: None,
            selected_diff: None,
            selected_diff_lines: Vec::new(),
            pending_line_discard: None,
            pending_hunk_discard: None,
            pending_discard: None,
            pending_operation_action: None,
            pending_branch_delete: None,
            confirm_dialog: None,
            pending_text_prompt: None,
            text_prompt_value: String::new(),
            pending_choice_prompt: None,
            choice_prompt_query: String::new(),
            choice_prompt_selected: 0,
            pull_dialog: None,
            push_dialog: None,
            stash_apply_dialog: None,
            show_command_palette: false,
            command_palette_query: String::new(),
            command_palette_selected: 0,
            show_about: false,
            pending_overlay_focus: None,
            selected_branch_review: None,
            branches_review_show_all: false,
            force_push_state: ForcePushState::Idle,
            shortcut_reference_state: ShortcutReferenceState::Hidden,
            commit_subject: String::new(),
            commit_body: String::new(),
            commit_amend: false,
            commit_amend_short_oid: None,
            commit_pre_amend_draft: None,
            commit_sign_off: false,
            author_identity: "Loading author identity…".into(),
            repository_view: RepositoryView::WorkingCopy,
            navigation_back: Vec::new(),
            navigation_forward: Vec::new(),
            came_from_welcome: false,
            history: Vec::new(),
            history_rows: Vec::new(),
            history_state: GraphState::default(),
            history_reference: HistoryReference::All,
            history_next: None,
            history_decorations: Vec::new(),
            selected_history: None,
            history_search: String::new(),
            history_list_state: ListState::new(0, ListAlignment::Top, px(72.0)),
            history_paths: Vec::new(),
            history_diff: None,
            history_selection_token: 0,
            history_load_token: 0,
            history_reveal_oid: None,
            history_detail_mode: HistoryDetailMode::Changeset,
            stashes: Vec::new(),
            stashes_load_token: 0,
            selected_stash: None,
            stash_selection_token: 0,
            selected_stash_paths: Vec::new(),
            selected_stash_diff: None,
            pending_stash_action_ref: None,
            reflog: Vec::new(),
            reflog_load_token: 0,
            selected_reflog: None,
            file_history: Vec::new(),
            file_history_path: String::new(),
            file_history_load_token: 0,
            blame: Vec::new(),
            blame_path: String::new(),
            blame_load_token: 0,
            compare_diff: None,
            compare_left: String::new(),
            compare_right: String::new(),
            compare_load_token: 0,
            tree: Vec::new(),
            tree_oid: String::new(),
            tree_path: Vec::new(),
            tree_blob: None,
            tree_blob_path: None,
            tree_load_token: 0,
            worktrees: Vec::new(),
            worktrees_load_token: 0,
            submodules: Vec::new(),
            submodules_load_token: 0,
            lfs: Vec::new(),
            lfs_load_token: 0,
            service_auth_state: git_domain::ServiceAuthState::SignedOut,
            service_account: None,
            hosted_repositories: Vec::new(),
            github_load_token: 0,
            pull_requests: Vec::new(),
            selected_pull_request: None,
            pull_request_detail: None,
            pull_request_repository: None,
            pull_requests_load_token: 0,
            pull_request_detail_token: 0,
            rebase_plan: Vec::new(),
            rebase_plan_load_token: 0,
            conflict_path: None,
            conflict_content: None,
            mutation_in_flight: false,
            network_operation: None,
            watcher: None,
            watch_events: None,
            store,
            diagnostics: "Checking Git installation…".into(),
            subscriptions: Vec::new(),
            column_width,
            welcome_search_input,
            worktree_search_input,
            commit_subject_input,
            commit_body_input,
            text_prompt_input,
            command_palette_input,
            choice_prompt_input,
            show_quick_open: false,
            welcome_plus_menu_open: false,
            last_commit_summary: None,
            file_diff_stats: std::collections::HashMap::new(),
        };
        app.observe_system_appearance(window, cx);
        app.observe_window_geometry(window, cx);
        app.observe_commit_composer_focus(window, cx);
        Self::load_diagnostics(cx);
        if let ShellState::Repository(repository) = &app.state {
            let repository = repository.clone();
            app.load_working_copy(repository.clone(), cx);
            Self::load_refs(repository.clone(), cx);
            Self::load_author_identity(repository.clone(), cx);
            app.start_watcher(&repository);
            Self::schedule_poll(repository, cx);
        }
        app
    }

    fn observe_system_appearance(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.subscriptions
            .push(cx.observe_window_appearance(window, |app, window, cx| {
                if app.theme_mode == ThemeMode::System {
                    app.appearance = appearance_from_window(window.appearance());
                    cx.notify();
                }
            }));
    }

    fn observe_window_geometry(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.subscriptions
            .push(cx.observe_window_bounds(window, |app, window, _| {
                let bounds = window.window_bounds().get_bounds();
                let _ = app.store.save_window_geometry(WindowGeometry {
                    x: bounds.origin.x.into(),
                    y: bounds.origin.y.into(),
                    width: bounds.size.width.into(),
                    height: bounds.size.height.into(),
                });
            }));
    }

    /// Update the status line and append to the capped activity history.
    ///
    /// Consecutive identical lines are skipped. Consecutive working-copy refresh
    /// chatter is coalesced so successes, errors, and confirmations stay visible.
    pub(crate) fn set_activity(&mut self, message: impl AsRef<str>) {
        let message = message.as_ref().to_owned();
        let kind = classify_activity(&message);
        let now = SystemTime::now();
        if is_working_copy_refresh_noise(&message)
            && self
                .activity_log
                .front()
                .is_some_and(|entry| is_working_copy_refresh_noise(&entry.message))
        {
            if let Some(front) = self.activity_log.front_mut() {
                front.message.clone_from(&message);
                front.kind = kind;
                front.at = now;
            }
        } else if self
            .activity_log
            .front()
            .is_none_or(|entry| entry.message != message)
        {
            self.activity_log.push_front(ActivityLogEntry {
                message: message.clone(),
                kind,
                at: now,
            });
            while self.activity_log.len() > ACTIVITY_LOG_CAPACITY {
                self.activity_log.pop_back();
            }
        }
        self.activity = message;
    }

    pub(crate) fn toggle_activity_log(&mut self, cx: &mut Context<Self>) {
        self.show_activity_log = !self.show_activity_log;
        cx.notify();
    }

    pub(crate) fn close_activity_log(&mut self, cx: &mut Context<Self>) {
        if self.show_activity_log {
            self.show_activity_log = false;
            cx.notify();
        }
    }

    fn observe_commit_composer_focus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let subject = self.commit_subject_input.focus_handle(cx);
        let body = self.commit_body_input.focus_handle(cx);
        self.subscriptions
            .push(cx.on_focus_in(&subject, window, |app, _, cx| {
                app.commit_subject_focused = true;
                app.commit_composer_expanded = true;
                cx.notify();
            }));
        self.subscriptions
            .push(cx.on_blur(&subject, window, |app, window, cx| {
                app.commit_subject_focused = false;
                // Defer so Amend/Sign-off clicks can update flags before collapse.
                cx.defer_in(window, |app, _, cx| {
                    app.sync_commit_composer_expanded(cx);
                });
            }));
        self.subscriptions
            .push(cx.on_focus_in(&body, window, |app, _, cx| {
                app.commit_body_focused = true;
                app.commit_composer_expanded = true;
                cx.notify();
            }));
        self.subscriptions
            .push(cx.on_blur(&body, window, |app, window, cx| {
                app.commit_body_focused = false;
                cx.defer_in(window, |app, _, cx| {
                    app.sync_commit_composer_expanded(cx);
                });
            }));
    }

    fn sync_commit_composer_expanded(&mut self, cx: &mut Context<Self>) {
        let keep_open = self.commit_subject_focused
            || self.commit_body_focused
            || self.commit_amend
            || self.commit_sign_off
            || !self.commit_subject.trim().is_empty()
            || !self.commit_body.trim().is_empty();
        if self.commit_composer_expanded != keep_open {
            self.commit_composer_expanded = keep_open;
            cx.notify();
        }
    }

    fn load_diagnostics(cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let diagnostics = cx
                .background_spawn(async {
                    GitExecutable::discover()
                        .and_then(|git| git.version())
                        .unwrap_or_else(|_| "Git was not found. Choose an installed Git executable before opening a repository.".into())
                })
                .await;
            let _ = this.update(cx, |app, cx| {
                app.diagnostics = diagnostics;
                cx.notify();
            });
        })
        .detach();
    }

    #[allow(clippy::too_many_lines)]
    fn apply_open_outcome(
        &mut self,
        outcome: Result<OpenedRepository, RepositoryOpenError>,
        cx: &mut Context<Self>,
    ) {
        match outcome {
            Ok(opened) => {
                self.state = ShellState::Repository(opened.repository);
                self.recents = opened.recents;
                self.selected_recent = None;
                self.set_activity("Repository opened.");
                self.working_copy = None;
                self.refs = RefSnapshot::default();
                self.ref_context = None;
                self.ref_context_menu_position = None;
                self.ref_context_submenu = None;
                self.commit_context = None;
                self.commit_context_menu_position = None;
                if let ShellState::Repository(repository) = &self.state {
                    self.branch_organization = self
                        .store
                        .load_branch_organization(&repository.worktree_root)
                        .unwrap_or_default();
                    self.workflow = self
                        .store
                        .load_workflow(&repository.worktree_root)
                        .ok()
                        .flatten();
                } else {
                    self.branch_organization = BranchOrganization::default();
                    self.workflow = None;
                }
                self.pull_dialog = None;
                self.push_dialog = None;
                self.stash_apply_dialog = None;
                self.selected_paths.clear();
                self.file_list_select_all_toggle = None;
                self.context_path = None;
                self.loaded_diff = None;
                self.selected_diff = None;
                self.selected_diff_lines.clear();
                self.pending_line_discard = None;
                self.pending_hunk_discard = None;
                self.pending_discard = None;
                self.pending_operation_action = None;
                self.pending_branch_delete = None;
                self.confirm_dialog = None;
                self.force_push_state = ForcePushState::Idle;
                self.shortcut_reference_state = ShortcutReferenceState::Hidden;
                self.network_operation = None;
                self.commit_subject.clear();
                self.commit_body.clear();
                self.refresh_commit_inputs(cx);
                self.commit_amend = false;
                self.commit_amend_short_oid = None;
                self.commit_pre_amend_draft = None;
                self.commit_sign_off = false;
                self.repository_view = RepositoryView::WorkingCopy;
                self.navigation_back.clear();
                self.navigation_forward.clear();
                self.history.clear();
                self.history_rows.clear();
                self.history_state = GraphState::default();
                self.history_reference = HistoryReference::All;
                self.history_next = None;
                self.history_decorations.clear();
                self.selected_history = None;
                self.history_search.clear();
                self.worktree_file_search.clear();
                self.history_list_state.reset(0);
                self.history_paths.clear();
                self.history_diff = None;
                self.history_selection_token = self.history_selection_token.wrapping_add(1);
                self.history_load_token = self.history_load_token.wrapping_add(1);
                self.history_detail_mode = HistoryDetailMode::Changeset;
                self.stashes.clear();
                self.stashes_load_token = self.stashes_load_token.wrapping_add(1);
                self.selected_stash = None;
                self.stash_selection_token = self.stash_selection_token.wrapping_add(1);
                self.selected_stash_paths.clear();
                self.selected_stash_diff = None;
                self.pending_stash_action_ref = None;
                self.lfs.clear();
                self.lfs_load_token = self.lfs_load_token.wrapping_add(1);
                self.service_auth_state = git_domain::ServiceAuthState::SignedOut;
                self.service_account = None;
                self.hosted_repositories.clear();
                self.github_load_token = self.github_load_token.wrapping_add(1);
                self.pull_requests.clear();
                self.selected_pull_request = None;
                self.pull_request_detail = None;
                self.pull_request_repository = None;
                self.pull_requests_load_token = self.pull_requests_load_token.wrapping_add(1);
                self.pull_request_detail_token = self.pull_request_detail_token.wrapping_add(1);
                if let ShellState::Repository(repository) = &self.state {
                    let repository = repository.clone();
                    self.load_working_copy(repository.clone(), cx);
                    Self::load_refs(repository.clone(), cx);
                    Self::load_author_identity(repository.clone(), cx);
                    self.start_watcher(&repository);
                    Self::schedule_poll(repository, cx);
                }
            }
            Err(error) => {
                self.state = ShellState::Error(error.to_string());
                self.set_activity("Repository could not be opened.");
            }
        }
        cx.notify();
    }

    fn refresh(&mut self, _: &Refresh, _: &mut Window, cx: &mut Context<Self>) {
        self.last_action = Some(LastAction::Refresh);
        Self::load_diagnostics(cx);
        if let ShellState::Repository(repository) = &self.state {
            self.load_working_copy(repository.clone(), cx);
        } else {
            self.set_activity("Open a repository before refreshing its working copy.");
        }
        cx.notify();
    }

    pub(crate) fn set_welcome_shell_view(
        &mut self,
        view: WelcomeShellView,
        cx: &mut Context<Self>,
    ) {
        if matches!(self.state, ShellState::Repository(_)) {
            self.return_to_welcome(cx);
        }
        self.welcome_plus_menu_open = false;
        if self.welcome_shell_view == view {
            cx.notify();
            return;
        }
        self.welcome_shell_view = view;
        if view == WelcomeShellView::Workflow {
            self.reload_workflow_from_selection();
        }
        cx.notify();
    }

    pub(crate) fn open_workflow(&mut self, cx: &mut Context<Self>) {
        if matches!(self.state, ShellState::Repository(_)) {
            self.navigate_to(RepositoryView::Workflow, cx);
            return;
        }
        self.set_welcome_shell_view(WelcomeShellView::Workflow, cx);
    }

    fn workflow_target_path(&self) -> Option<PathBuf> {
        match &self.state {
            ShellState::Repository(repository) => Some(repository.worktree_root.clone()),
            _ => self
                .selected_recent
                .and_then(|index| self.recents.get(index).cloned()),
        }
    }

    fn reload_workflow_from_selection(&mut self) {
        self.workflow = self
            .workflow_target_path()
            .and_then(|path| self.store.load_workflow(&path).ok().flatten());
    }

    fn persist_workflow(&mut self, cx: &mut Context<Self>) {
        let Some(path) = self.workflow_target_path() else {
            self.set_activity("Select a repository before saving a workflow.");
            cx.notify();
            return;
        };
        if self
            .store
            .save_workflow(&path, self.workflow.as_ref())
            .is_err()
        {
            self.set_activity("Workflow could not be saved.");
        }
        cx.notify();
    }

    fn local_branch_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self
            .refs
            .local_branches
            .iter()
            .filter_map(|branch| String::from_utf8(branch.name.0.clone()).ok())
            .collect();
        if names.is_empty()
            && let Some(branch) = self
                .welcome_snapshot
                .as_ref()
                .and_then(|snapshot| snapshot.branch.clone())
        {
            names.push(branch);
        }
        names
    }

    pub(crate) fn apply_workflow_kind(&mut self, kind: WorkflowKind, cx: &mut Context<Self>) {
        if self.workflow_target_path().is_none() {
            self.set_activity("Select a repository before saving a workflow.");
            cx.notify();
            return;
        }
        let names = self.local_branch_names();
        self.workflow = Some(RepositoryWorkflow::from_kind(kind, &names));
        self.set_activity(format!("Applied {}.", kind.title()));
        self.persist_workflow(cx);
    }

    pub(crate) fn detect_workflow(&mut self, cx: &mut Context<Self>) {
        if self.workflow_target_path().is_none() {
            self.set_activity("Select a repository before saving a workflow.");
            cx.notify();
            return;
        }
        let names = self.local_branch_names();
        let workflow = RepositoryWorkflow::detect(&names);
        self.set_activity(format!("Detected {}.", workflow.kind.title()));
        self.workflow = Some(workflow);
        self.persist_workflow(cx);
    }

    pub(crate) fn disable_workflow(&mut self, cx: &mut Context<Self>) {
        self.workflow = None;
        self.set_activity("Workflow disabled for this repository.");
        self.persist_workflow(cx);
    }

    pub(crate) fn cycle_workflow_topic_strategy(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(workflow) = self.workflow.as_mut() else {
            return;
        };
        let Some(topic) = workflow.topics.get_mut(index) else {
            return;
        };
        topic.strategy = topic.strategy.next();
        workflow.kind = WorkflowKind::Custom;
        let label = topic.label.clone();
        let strategy = topic.strategy.title();
        self.set_activity(format!("{label} now finishes with {strategy}."));
        self.persist_workflow(cx);
    }

    pub(crate) fn prompt_start_workflow_topic(
        &mut self,
        prefix: String,
        start: String,
        cx: &mut Context<Self>,
    ) {
        if !matches!(self.state, ShellState::Repository(_)) {
            self.set_activity("Open the repository to start a topic branch.");
            cx.notify();
            return;
        }
        self.begin_text_prompt(TextPromptKind::StartWorkflowTopic { prefix, start }, "", cx);
    }

    pub(crate) fn prompt_finish_workflow_topic(&mut self, cx: &mut Context<Self>) {
        let Some(branch) = self.head_branch_name() else {
            self.set_activity("Check out a topic branch before finishing.");
            cx.notify();
            return;
        };
        let Some(workflow) = self.workflow.as_ref() else {
            self.set_activity("Apply a workflow template first.");
            cx.notify();
            return;
        };
        let Some(topic) = workflow.topic_for_branch(&branch) else {
            self.set_activity(format!("{branch} is not a configured topic branch."));
            cx.notify();
            return;
        };
        if topic.merge_into.is_empty() {
            self.set_activity("This topic type has no finish targets.");
            cx.notify();
            return;
        }
        self.confirm_dialog = Some(AppConfirmDialog::FinishTopic {
            branch,
            label: topic.label.clone(),
            targets: topic.merge_into.join(", "),
            strategy: topic.strategy.title().to_owned(),
        });
        self.set_activity("Confirm finishing the topic branch.");
        cx.notify();
    }

    fn finish_workflow_topic(&mut self, branch: &str, cx: &mut Context<Self>) {
        let Some(workflow) = self.workflow.as_ref() else {
            return;
        };
        let Some(topic) = workflow.topic_for_branch(branch).cloned() else {
            self.set_activity(format!("{branch} is not a configured topic branch."));
            cx.notify();
            return;
        };
        let steps = RepositoryWorkflow::finish_steps(branch, &topic, true);
        if steps.is_empty() {
            self.set_activity("Nothing to finish for this topic type.");
            cx.notify();
            return;
        }
        self.run_workflow_steps(format!("Finishing {branch}"), steps, cx);
    }

    pub(crate) fn sync_workflow_head(&mut self, cx: &mut Context<Self>) {
        let Some(head) = self.head_branch_name() else {
            self.set_activity("Check out a branch before syncing.");
            cx.notify();
            return;
        };
        let Some(parent) = self
            .workflow
            .as_ref()
            .and_then(|workflow| workflow.sync_parent(&head))
            .map(str::to_owned)
        else {
            self.set_activity(format!("{head} has no parent to sync from."));
            cx.notify();
            return;
        };
        self.run_workflow_steps(
            format!("Syncing {head} from {parent}"),
            vec![WorkflowGitStep::Merge(parent)],
            cx,
        );
    }

    fn run_workflow_steps(
        &mut self,
        label: String,
        steps: Vec<WorkflowGitStep>,
        cx: &mut Context<Self>,
    ) {
        self.run_branch_command(
            label,
            move |git, repository| {
                for step in &steps {
                    match step {
                        WorkflowGitStep::Checkout(name) => {
                            git.checkout_branch(repository, name)?;
                        }
                        WorkflowGitStep::Merge(name) => {
                            git.merge_branch(repository, name)?;
                        }
                        WorkflowGitStep::MergeSquash(name) => {
                            git.merge_squash(repository, name)?;
                        }
                        WorkflowGitStep::Rebase(name) => {
                            git.rebase_branch(repository, name)?;
                        }
                        WorkflowGitStep::MergeFfOnly(name) => {
                            git.merge_ff_only(repository, name)?;
                        }
                        WorkflowGitStep::DeleteBranch { name, force } => {
                            git.delete_branch(repository, name, *force)?;
                        }
                    }
                }
                Ok(())
            },
            cx,
        );
    }

    fn persist_bookmark_organization(&self) {
        let organization = BookmarkOrganization {
            folders: self.bookmark_folders.clone(),
            repository_folders: self
                .repository_folders
                .iter()
                .map(|(path, folder)| (path.to_string_lossy().into_owned(), folder.clone()))
                .collect(),
        };
        let _ = self.store.save_bookmark_organization(&organization);
    }

    fn create_bookmark_folder(&mut self, name: &str, cx: &mut Context<Self>) {
        let name = name.trim().to_owned();
        if name.is_empty() {
            self.set_activity("Enter a folder name.");
            cx.notify();
            return;
        }
        let id = format!(
            "folder-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_millis())
        );
        self.bookmark_folders.push(BookmarkFolder {
            id,
            name,
            expanded: true,
        });
        self.persist_bookmark_organization();
        self.set_activity("Folder created.");
        cx.notify();
    }

    fn rename_bookmark_folder(&mut self, id: &str, name: &str, cx: &mut Context<Self>) {
        let name = name.trim().to_owned();
        if name.is_empty() {
            self.set_activity("Enter a folder name.");
            cx.notify();
            return;
        }
        let Some(folder) = self
            .bookmark_folders
            .iter_mut()
            .find(|folder| folder.id == id)
        else {
            return;
        };
        folder.name = name;
        self.persist_bookmark_organization();
        self.set_activity("Folder renamed.");
        cx.notify();
    }

    fn delete_bookmark_folder(&mut self, id: &str, cx: &mut Context<Self>) {
        self.bookmark_folders.retain(|folder| folder.id != id);
        self.repository_folders
            .retain(|_, folder_id| folder_id.as_str() != id);
        self.persist_bookmark_organization();
        self.set_activity("Folder deleted. Repositories are now at the root.");
        cx.notify();
    }

    pub(crate) fn toggle_bookmark_folder(&mut self, id: &str, cx: &mut Context<Self>) {
        let Some(folder) = self
            .bookmark_folders
            .iter_mut()
            .find(|folder| folder.id == id)
        else {
            return;
        };
        folder.expanded = !folder.expanded;
        self.persist_bookmark_organization();
        cx.notify();
    }

    pub(crate) fn move_repository_to_folder(
        &mut self,
        path: PathBuf,
        folder_id: Option<String>,
        cx: &mut Context<Self>,
    ) {
        match folder_id {
            Some(folder_id)
                if self
                    .bookmark_folders
                    .iter()
                    .any(|folder| folder.id == folder_id) =>
            {
                self.repository_folders.insert(path, folder_id);
            }
            _ => {
                self.repository_folders.remove(&path);
            }
        }
        self.persist_bookmark_organization();
        cx.notify();
    }

    #[allow(dead_code)]
    fn set_welcome_repo_search(&mut self, query: String, cx: &mut Context<Self>) {
        self.welcome_repo_search = query;
        cx.notify();
    }

    #[allow(dead_code)]
    fn set_worktree_file_search(&mut self, query: String, cx: &mut Context<Self>) {
        self.worktree_file_search = query;
        cx.notify();
    }

    fn prompt_create_repository(&mut self, cx: &mut Context<Self>) {
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Choose a folder for the new repository".into()),
        });
        let store = self.store.clone();
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(paths))) = receiver.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            let outcome = cx
                .background_spawn(async move {
                    let git = GitExecutable::discover()
                        .map_err(|_| RepositoryOpenError::DiscoveryFailed)?;
                    git.init_repository(&path)
                        .map_err(|_| RepositoryOpenError::DiscoveryFailed)?;
                    discover_and_record(&path, &store)
                })
                .await;
            let _ = this.update(cx, |app, cx| app.apply_open_outcome(outcome, cx));
        })
        .detach();
    }

    #[allow(clippy::too_many_lines)]
    fn prompt_clone_repository(&mut self, cx: &mut Context<Self>) {
        const SCRIPT: &str = r#"set remote_url to text returned of (display dialog "Clone URL or local path" default answer "")
set parent_folder to choose folder with prompt "Choose destination parent folder"
set parent_path to POSIX path of parent_folder
return remote_url & linefeed & parent_path"#;
        let store = self.store.clone();
        cx.spawn(async move |this, cx| {
            let choice = cx
                .background_spawn(async {
                    let output = Command::new("osascript")
                        .args(["-e", SCRIPT])
                        .output()
                        .ok()
                        .filter(|output| output.status.success())?;
                    let text = String::from_utf8_lossy(&output.stdout);
                    let mut lines = text.lines();
                    let source = lines.next()?.trim().to_owned();
                    let parent = lines.next()?.trim().to_owned();
                    (!source.is_empty() && !parent.is_empty()).then_some((source, parent))
                })
                .await;
            let Some((source, parent)) = choice else {
                return;
            };
            let name = source
                .trim_end_matches('/')
                .rsplit('/')
                .find(|part| !part.is_empty())
                .unwrap_or("repository")
                .trim_end_matches(".git");
            let destination = PathBuf::from(parent).join(name);
            let outcome = cx
                .background_spawn(async move {
                    let git = GitExecutable::discover()
                        .map_err(|_| RepositoryOpenError::DiscoveryFailed)?;
                    git.clone_repository(&source, &destination)
                        .map_err(|_| RepositoryOpenError::DiscoveryFailed)?;
                    discover_and_record(&destination, &store)
                })
                .await;
            let _ = this.update(cx, |app, cx| app.apply_open_outcome(outcome, cx));
        })
        .detach();
    }

    fn focus_composer(&mut self, _: &FocusComposer, window: &mut Window, cx: &mut Context<Self>) {
        self.edit_commit_subject(window, cx);
    }

    fn focus_search(&mut self, _: &FocusSearch, window: &mut Window, cx: &mut Context<Self>) {
        if self.pending_text_prompt.is_some()
            || self.pending_choice_prompt.is_some()
            || self.show_command_palette
            || self.show_quick_open
        {
            return;
        }
        let handle = match &self.state {
            ShellState::Welcome => self.welcome_search_input.focus_handle(cx),
            ShellState::Repository(_) => self.worktree_search_input.focus_handle(cx),
            ShellState::Loading(_) | ShellState::Error(_) => return,
        };
        window.focus(&handle);
        cx.notify();
    }

    fn select_all_status_files(
        &mut self,
        _: &SelectAllStatusFiles,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.repository_view != RepositoryView::WorkingCopy
            || self.pending_text_prompt.is_some()
            || self.show_command_palette
            || self.show_quick_open
        {
            return;
        }
        let visible = self.visible_status_paths();
        if visible.is_empty() {
            return;
        }
        self.selected_paths = visible;
        self.file_list_select_all_toggle = None;
        self.last_selected_path_index = Some(0);
        self.selected_diff = None;
        self.loaded_diff = None;
        cx.notify();
    }

    fn save_stash_shortcut(&mut self, _: &SaveStash, _: &mut Window, cx: &mut Context<Self>) {
        if !matches!(self.state, ShellState::Repository(_))
            || self.pending_text_prompt.is_some()
            || self.stash_apply_dialog.is_some()
            || self.show_command_palette
            || self.show_quick_open
        {
            return;
        }
        self.open_stash_save_dialog(false, Vec::new(), cx);
    }

    fn show_command_palette(&mut self, _: &CommandPalette, _: &mut Window, cx: &mut Context<Self>) {
        self.open_command_palette(cx);
    }

    pub(crate) fn open_command_palette(&mut self, cx: &mut Context<Self>) {
        self.show_quick_open = false;
        self.welcome_plus_menu_open = false;
        self.pending_text_prompt = None;
        self.text_prompt_value.clear();
        self.pending_choice_prompt = None;
        self.choice_prompt_query.clear();
        self.choice_prompt_selected = 0;
        self.show_command_palette = true;
        self.command_palette_query.clear();
        self.command_palette_selected = 0;
        self.pending_overlay_focus = Some(OverlayFocus::CommandPalette);
        self.set_activity("Choose a command from the palette.");
        cx.notify();
    }

    fn close_command_palette(&mut self, cx: &mut Context<Self>) {
        self.show_command_palette = false;
        self.command_palette_query.clear();
        self.command_palette_selected = 0;
        cx.notify();
    }

    fn show_about(&mut self, _: &About, _: &mut Window, cx: &mut Context<Self>) {
        self.show_about_dialog(cx);
    }

    pub(crate) fn show_about_dialog(&mut self, cx: &mut Context<Self>) {
        self.show_about = true;
        cx.notify();
    }

    pub(crate) fn close_about_dialog(&mut self, cx: &mut Context<Self>) {
        if self.show_about {
            self.show_about = false;
            cx.notify();
        }
    }

    fn move_command_palette_selection(&mut self, delta: isize, cx: &mut Context<Self>) {
        let count = PaletteCommand::filtered(&self.command_palette_query).len();
        if count == 0 {
            self.command_palette_selected = 0;
            cx.notify();
            return;
        }
        let current = self.command_palette_selected.min(count - 1);
        let next = match delta.cmp(&0) {
            std::cmp::Ordering::Less => current.saturating_sub(delta.unsigned_abs()),
            std::cmp::Ordering::Greater => (current + delta.unsigned_abs()).min(count - 1),
            std::cmp::Ordering::Equal => current,
        };
        self.command_palette_selected = next;
        cx.notify();
    }

    fn confirm_command_palette(&mut self, cx: &mut Context<Self>) {
        let commands = PaletteCommand::filtered(&self.command_palette_query);
        let Some((_, _, command)) = commands
            .get(
                self.command_palette_selected
                    .min(commands.len().saturating_sub(1)),
            )
            .copied()
        else {
            return;
        };
        self.run_palette_command(command, cx);
    }

    #[allow(clippy::too_many_lines)]
    pub(crate) fn run_palette_command(&mut self, command: PaletteCommand, cx: &mut Context<Self>) {
        self.close_command_palette(cx);
        match command {
            PaletteCommand::OpenRepository => self.add_repository_from_picker(cx),
            PaletteCommand::Fetch => self.fetch_default_remote(cx),
            PaletteCommand::Pull => self.pull_current(cx),
            PaletteCommand::Push => self.push_current(cx),
            PaletteCommand::Sync => self.sync_current(cx),
            PaletteCommand::RefreshWorkingCopy => {
                if let ShellState::Repository(repository) = &self.state {
                    self.load_working_copy(repository.clone(), cx);
                } else {
                    self.set_activity("Open a repository before refreshing its working copy.");
                    cx.notify();
                }
            }
            PaletteCommand::ShowWorkingCopy => {
                self.navigate_to(RepositoryView::WorkingCopy, cx);
            }
            PaletteCommand::StageAll => {
                if matches!(self.state, ShellState::Repository(_)) {
                    self.run_mutation(Mutation::StageAll, Vec::new(), false, cx);
                } else {
                    self.set_activity("Open a repository before staging.");
                    cx.notify();
                }
            }
            PaletteCommand::UnstageAll => {
                if matches!(self.state, ShellState::Repository(_)) {
                    self.run_mutation(Mutation::UnstageAll, Vec::new(), false, cx);
                } else {
                    self.set_activity("Open a repository before unstaging.");
                    cx.notify();
                }
            }
            PaletteCommand::FocusCommitComposer => {
                self.navigate_to(RepositoryView::WorkingCopy, cx);
                self.commit_subject_focused = true;
                self.commit_composer_expanded = true;
                self.set_activity("Commit composer ready — type a subject.");
                cx.notify();
            }
            PaletteCommand::AmendLastCommit => {
                self.navigate_to(RepositoryView::WorkingCopy, cx);
                self.toggle_commit_amend(cx);
            }
            PaletteCommand::SaveStash => self.open_stash_save_dialog(false, Vec::new(), cx),
            PaletteCommand::SaveStashUntracked => self.open_stash_save_dialog(true, Vec::new(), cx),
            PaletteCommand::ApplyLatestStash => self.open_apply_latest_stash_dialog(cx),
            PaletteCommand::ApplySelectedStash => self.open_stash_apply_dialog_for_selection(cx),
            PaletteCommand::BranchFromSelectedStash => self.prompt_branch_from_selected_stash(cx),
            PaletteCommand::PopSelectedStash => self.request_stash_action(StashAction::Pop, cx),
            PaletteCommand::DropSelectedStash => self.request_stash_action(StashAction::Drop, cx),
            PaletteCommand::CreateBranch => {
                self.begin_text_prompt(TextPromptKind::CreateBranch { start: None }, "", cx);
            }
            PaletteCommand::CreateTag => {
                let start = self.selected_history_oid().unwrap_or_else(|| "HEAD".into());
                self.begin_text_prompt(TextPromptKind::CreateTag { start }, "", cx);
            }
            PaletteCommand::ShowHistory => {
                if let ShellState::Repository(repository) = &self.state {
                    self.show_history(repository.clone(), cx);
                }
            }
            PaletteCommand::HistoryFilter => {
                self.begin_choice_prompt(ChoicePromptKind::HistoryFilter, cx);
            }
            PaletteCommand::RevealHistoryHead => self.reveal_history_head(cx),
            PaletteCommand::CopySelectedCommitHash => self.copy_selected_history_oid(cx),
            PaletteCommand::BranchFromSelectedCommit => self.prompt_branch_from_selected(cx),
            PaletteCommand::CheckoutSelectedCommit
            | PaletteCommand::ResetHeadToSelected
            | PaletteCommand::RevertSelectedCommit
            | PaletteCommand::SaveSelectedPatch
            | PaletteCommand::ExportSelectedCommit
            | PaletteCommand::CompareSelectedCommit => {
                self.run_selected_history_palette_command(command, cx);
            }
            PaletteCommand::CommitDetail => {
                if let ShellState::Repository(repository) = &self.state {
                    if let Some(index) = self.selected_history {
                        let repository = repository.clone();
                        self.show_commit_detail(&repository, index, cx);
                    } else {
                        self.set_activity("Select a history commit first.");
                        cx.notify();
                    }
                }
            }
            PaletteCommand::ShowStashes => {
                if let ShellState::Repository(repository) = &self.state {
                    self.show_stashes(repository.clone(), cx);
                }
            }
            PaletteCommand::ShowRemotes => {
                self.show_remotes(cx);
            }
            PaletteCommand::ShowSettings => {
                self.navigate_to(RepositoryView::Settings, cx);
            }
            PaletteCommand::ShowWorkflow => self.open_workflow(cx),
            PaletteCommand::ShowPullRequests => self.open_pull_requests(cx),
            PaletteCommand::ShowBranchesReview => {
                self.navigate_to(RepositoryView::BranchesReview, cx);
            }
            PaletteCommand::GitLfsStatus => {
                if let ShellState::Repository(repository) = &self.state {
                    self.show_lfs(repository.clone(), cx);
                }
            }
            PaletteCommand::ShowReflog => {
                if let ShellState::Repository(repository) = &self.state {
                    self.show_reflog(repository.clone(), cx);
                }
            }
            PaletteCommand::FileHistory => self.prompt_file_history(cx),
            PaletteCommand::Blame => self.prompt_blame(cx),
            PaletteCommand::CompareRefs => self.prompt_compare_refs(cx),
            PaletteCommand::BrowseTree => self.prompt_browse_tree(cx),
            PaletteCommand::Worktrees => {
                if let ShellState::Repository(repository) = &self.state {
                    self.show_worktrees(repository.clone(), cx);
                }
            }
            PaletteCommand::Submodules => {
                if let ShellState::Repository(repository) = &self.state {
                    self.show_submodules(repository.clone(), cx);
                }
            }
            PaletteCommand::RebasePlan => {
                if let ShellState::Repository(repository) = &self.state {
                    self.show_rebase(repository.clone(), cx);
                }
            }
            PaletteCommand::RebaseOnto => self.prompt_start_rebase(cx),
            PaletteCommand::MergeRevision => {
                self.begin_text_prompt(TextPromptKind::MergeRevision, "", cx);
            }
            PaletteCommand::SquashStaged => self.prompt_autosquash(true, cx),
            PaletteCommand::FixupStaged => self.prompt_autosquash(false, cx),
            PaletteCommand::DropCommit => self.prompt_drop_commit(cx),
            PaletteCommand::RewordLastCommit => self.prompt_reword_last_commit(cx),
            PaletteCommand::Conflicts => {
                if let ShellState::Repository(repository) = &self.state {
                    self.show_conflicts(repository.clone(), cx);
                }
            }
            PaletteCommand::SetMergeTool => self.prompt_set_merge_tool(cx),
            PaletteCommand::OpenInMergeTool => self.prompt_run_merge_tool(cx),
            PaletteCommand::CheckCommitSignature => Self::prompt_check_commit_signature(cx),
            PaletteCommand::QuickOpenFile => {
                self.show_quick_open = true;
                self.pending_overlay_focus = None;
                cx.notify();
            }
            PaletteCommand::ToggleMessageHistory => self.toggle_activity_log(cx),
            PaletteCommand::ToggleAppearance => {
                self.theme_mode = match self.theme_mode {
                    ThemeMode::System => ThemeMode::Light,
                    ThemeMode::Light => ThemeMode::Dark,
                    ThemeMode::Dark => ThemeMode::System,
                };
                self.apply_theme_mode(None, cx);
            }
            PaletteCommand::ShowKeyboardShortcuts => {
                self.shortcut_reference_state = ShortcutReferenceState::Visible;
                cx.notify();
            }
            PaletteCommand::NavigateBack => {
                if self.came_from_welcome {
                    self.came_from_welcome = false;
                    self.navigation_forward.clear();
                    self.state = ShellState::Welcome;
                    self.working_copy = None;
                    self.refs = RefSnapshot::default();
                    cx.notify();
                } else if let Some(view) = self.navigation_back.pop() {
                    self.navigation_forward.push(self.repository_view);
                    self.repository_view = view;
                    cx.notify();
                }
            }
            PaletteCommand::NavigateForward => {
                if let Some(view) = self.navigation_forward.pop() {
                    self.navigation_back.push(self.repository_view);
                    self.repository_view = view;
                    cx.notify();
                }
            }
            PaletteCommand::AboutGitRonimo => self.show_about_dialog(cx),
        }
    }

    fn selected_history_oid(&self) -> Option<String> {
        self.selected_history
            .and_then(|index| self.history.get(index))
            .map(|commit| commit.oid.clone())
    }

    fn run_selected_history_palette_command(
        &mut self,
        command: PaletteCommand,
        cx: &mut Context<Self>,
    ) {
        let Some(oid) = self.selected_history_oid() else {
            self.set_activity("Select a history commit first.");
            cx.notify();
            return;
        };
        match command {
            PaletteCommand::CheckoutSelectedCommit => self.checkout_detached_commit(&oid, cx),
            PaletteCommand::ResetHeadToSelected => {
                if !self.history_is_head_branch_scope() {
                    self.set_activity(
                        "Reset is available only when History shows the current HEAD branch.",
                    );
                    cx.notify();
                    return;
                }
                self.prompt_reset_head_to(&oid, cx);
            }
            PaletteCommand::RevertSelectedCommit => {
                if !self.history_is_head_branch_scope() {
                    self.set_activity(
                        "Revert is available only when History shows the current HEAD branch.",
                    );
                    cx.notify();
                    return;
                }
                self.prompt_revert_commit(&oid, cx);
            }
            PaletteCommand::SaveSelectedPatch => self.save_commit_patch(&oid, cx),
            PaletteCommand::ExportSelectedCommit => self.export_branch_archive(&oid, cx),
            PaletteCommand::CompareSelectedCommit => {
                self.begin_text_prompt(TextPromptKind::CompareTo { left: oid }, "", cx);
            }
            _ => {}
        }
    }

    fn toggle_shortcut_reference(
        &mut self,
        _: &ShortcutReference,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.shortcut_reference_state = match self.shortcut_reference_state {
            ShortcutReferenceState::Hidden => ShortcutReferenceState::Visible,
            ShortcutReferenceState::Visible => ShortcutReferenceState::Hidden,
        };
        cx.notify();
    }

    fn history_previous(&mut self, _: &HistoryPrevious, _: &mut Window, cx: &mut Context<Self>) {
        if self.repository_view == RepositoryView::Reflog {
            self.move_reflog_selection(-1, cx);
        } else if self.repository_view == RepositoryView::Stashes {
            self.move_stash_selection(-1, cx);
        } else {
            self.move_history_selection(-1, cx);
        }
    }

    fn history_next(&mut self, _: &HistoryNext, _: &mut Window, cx: &mut Context<Self>) {
        if self.repository_view == RepositoryView::Reflog {
            self.move_reflog_selection(1, cx);
        } else if self.repository_view == RepositoryView::Stashes {
            self.move_stash_selection(1, cx);
        } else {
            self.move_history_selection(1, cx);
        }
    }

    fn navigate_back(&mut self, _: &NavigateBack, _: &mut Window, cx: &mut Context<Self>) {
        if self.came_from_welcome {
            self.came_from_welcome = false;
            self.navigation_forward.clear();
            self.state = ShellState::Welcome;
            self.working_copy = None;
            self.refs = RefSnapshot::default();
            cx.notify();
            return;
        }
        let Some(view) = self.navigation_back.pop() else {
            return;
        };
        self.navigation_forward.push(self.repository_view);
        self.repository_view = view;
        cx.notify();
    }

    fn navigate_forward(&mut self, _: &NavigateForward, _: &mut Window, cx: &mut Context<Self>) {
        let Some(view) = self.navigation_forward.pop() else {
            return;
        };
        self.navigation_back.push(self.repository_view);
        self.repository_view = view;
        cx.notify();
    }

    pub(crate) fn navigate_to(&mut self, view: RepositoryView, cx: &mut Context<Self>) {
        if self.repository_view != view {
            self.navigation_back.push(self.repository_view);
            self.navigation_forward.clear();
            self.repository_view = view;
            cx.notify();
        }
    }

    #[allow(dead_code)]
    fn return_to_welcome(&mut self, cx: &mut Context<Self>) {
        if matches!(self.state, ShellState::Welcome) {
            return;
        }
        self.navigation_forward.clear();
        self.navigation_back.clear();
        self.came_from_welcome = false;
        self.state = ShellState::Welcome;
        self.working_copy = None;
        self.refs = RefSnapshot::default();
        self.repository_view = RepositoryView::WorkingCopy;
        self.selected_recent = (!self.recents.is_empty()).then_some(0);
        self.welcome_snapshot = None;
        self.welcome_snapshot_path = None;
        self.welcome_list_snapshots.clear();
        self.reload_workflow_from_selection();
        if self.selected_recent.is_some() {
            self.refresh_welcome_snapshot(cx);
        }
        self.refresh_welcome_list_snapshots(cx);
        cx.notify();
    }

    fn request_branch_delete(&mut self, branch: &str, cx: &mut Context<Self>) {
        self.close_ref_context_menu(cx);
        self.confirm_dialog = None;
        self.pending_branch_delete = Some(branch.to_owned());
        self.set_activity(format!("Confirm deletion of branch {branch}."));
        cx.notify();
    }

    pub(crate) fn cancel_branch_delete(&mut self, cx: &mut Context<Self>) {
        self.pending_branch_delete = None;
        self.set_activity("Branch deletion cancelled.");
        cx.notify();
    }

    pub(crate) fn cancel_confirm_dialog(&mut self, cx: &mut Context<Self>) {
        self.confirm_dialog = None;
        self.set_activity("Cancelled.");
        cx.notify();
    }

    pub(crate) fn confirm_confirm_dialog(&mut self, cx: &mut Context<Self>) {
        let Some(dialog) = self.confirm_dialog.take() else {
            return;
        };
        match dialog {
            AppConfirmDialog::ForceDeleteBranch { branch } => {
                self.pending_branch_delete = Some(branch);
                self.confirm_branch_delete(true, cx);
            }
            AppConfirmDialog::HardReset { oid } => {
                self.run_reset_to(&oid, ResetMode::Hard, cx);
            }
            AppConfirmDialog::RevertCommit { oid } => {
                self.run_worktree_mutation(
                    format!("Revert {}", oid.chars().take(8).collect::<String>()),
                    move |git, repository| git.revert_commit(repository, &oid),
                    cx,
                );
            }
            AppConfirmDialog::DropCommit { oid } => {
                self.run_worktree_mutation(
                    format!("Drop {}", oid.chars().take(8).collect::<String>()),
                    move |git, repository| git.drop_commit(repository, &oid),
                    cx,
                );
            }
            AppConfirmDialog::FinishTopic { branch, .. } => {
                self.finish_workflow_topic(&branch, cx);
            }
        }
    }

    pub(crate) fn begin_text_prompt(
        &mut self,
        kind: TextPromptKind,
        initial: impl Into<String>,
        cx: &mut Context<Self>,
    ) {
        self.show_command_palette = false;
        self.show_quick_open = false;
        self.welcome_plus_menu_open = false;
        self.pending_choice_prompt = None;
        self.choice_prompt_query.clear();
        self.choice_prompt_selected = 0;
        self.pending_text_prompt = Some(kind);
        self.text_prompt_value = initial.into();
        self.pending_overlay_focus = Some(OverlayFocus::TextPrompt);
        cx.notify();
    }

    fn cancel_text_prompt(&mut self, cx: &mut Context<Self>) {
        self.pending_text_prompt = None;
        self.text_prompt_value.clear();
        cx.notify();
    }

    #[allow(clippy::too_many_lines)]
    fn confirm_text_prompt(&mut self, cx: &mut Context<Self>) {
        let Some(kind) = self.pending_text_prompt.clone() else {
            return;
        };
        let value = self.text_prompt_value.trim().to_owned();
        match kind {
            TextPromptKind::BranchRename { current } => {
                if value.is_empty() || value == current {
                    self.set_activity("Enter a new branch name to rename.");
                    cx.notify();
                    return;
                }
                self.pending_text_prompt = None;
                self.text_prompt_value.clear();
                self.run_branch_command(
                    format!("Renaming branch to {value}"),
                    move |git, repository| git.rename_branch(repository, &current, &value),
                    cx,
                );
            }
            TextPromptKind::CreateBranch { start } => {
                if value.is_empty() {
                    self.set_activity("Enter a branch name.");
                    cx.notify();
                    return;
                }
                self.pending_text_prompt = None;
                self.text_prompt_value.clear();
                self.create_branch_from(value, start, cx);
            }
            TextPromptKind::StartWorkflowTopic { prefix, start } => {
                if value.is_empty() {
                    self.set_activity("Enter a topic branch name.");
                    cx.notify();
                    return;
                }
                self.pending_text_prompt = None;
                self.text_prompt_value.clear();
                let branch = RepositoryWorkflow::branch_name_for_topic(&prefix, &value);
                self.create_branch_from(branch, Some(start), cx);
            }
            TextPromptKind::CreateTag { start } => {
                if value.is_empty() {
                    self.set_activity("Enter a tag name.");
                    cx.notify();
                    return;
                }
                self.pending_text_prompt = None;
                self.text_prompt_value.clear();
                self.run_branch_command(
                    format!("Creating tag {value}"),
                    move |git, repository| git.create_tag(repository, &value, &start),
                    cx,
                );
            }
            TextPromptKind::CreateStash {
                include_untracked,
                paths,
            } => {
                self.pending_text_prompt = None;
                self.text_prompt_value.clear();
                let message = (!value.is_empty()).then_some(value);
                self.run_create_stash(
                    CreateStashRequest {
                        message,
                        include_untracked,
                        paths,
                    },
                    cx,
                );
            }
            TextPromptKind::StashBranch { reference } => {
                if value.is_empty() {
                    self.set_activity("Enter a branch name.");
                    cx.notify();
                    return;
                }
                self.pending_text_prompt = None;
                self.text_prompt_value.clear();
                self.run_stash_branch(value, reference, cx);
            }
            TextPromptKind::FileHistoryPath => {
                if value.is_empty() {
                    self.set_activity("Enter a repository path.");
                    cx.notify();
                    return;
                }
                self.pending_text_prompt = None;
                self.text_prompt_value.clear();
                self.file_history_path = value;
                let ShellState::Repository(repository) = &self.state else {
                    return;
                };
                let repository = repository.clone();
                self.navigate_to(RepositoryView::FileHistory, cx);
                self.load_file_history(repository, cx);
            }
            TextPromptKind::BlamePath => {
                if value.is_empty() {
                    self.set_activity("Enter a repository path.");
                    cx.notify();
                    return;
                }
                self.pending_text_prompt = None;
                self.text_prompt_value.clear();
                self.blame_path = value;
                let ShellState::Repository(repository) = &self.state else {
                    return;
                };
                let repository = repository.clone();
                self.navigate_to(RepositoryView::Blame, cx);
                self.load_blame(repository, cx);
            }
            TextPromptKind::CompareFrom => {
                if value.is_empty() {
                    self.set_activity("Enter a ref to compare from.");
                    cx.notify();
                    return;
                }
                self.text_prompt_value.clear();
                self.pending_text_prompt = Some(TextPromptKind::CompareTo { left: value });
                self.pending_overlay_focus = Some(OverlayFocus::TextPrompt);
                cx.notify();
            }
            TextPromptKind::CompareTo { left } => {
                if value.is_empty() {
                    self.set_activity("Enter a ref to compare to.");
                    cx.notify();
                    return;
                }
                self.pending_text_prompt = None;
                self.text_prompt_value.clear();
                self.compare_left = left;
                self.compare_right = value;
                let ShellState::Repository(repository) = &self.state else {
                    return;
                };
                let repository = repository.clone();
                self.navigate_to(RepositoryView::Compare, cx);
                self.load_compare(repository, cx);
            }
            TextPromptKind::DropCommit => {
                if value.is_empty() {
                    self.set_activity("Enter a commit to drop (e.g. HEAD~1).");
                    cx.notify();
                    return;
                }
                self.pending_text_prompt = None;
                self.text_prompt_value.clear();
                self.run_worktree_mutation(
                    format!("Drop {value}"),
                    move |git, repository| git.drop_commit(repository, &value),
                    cx,
                );
            }
            TextPromptKind::BrowseTree => {
                if value.is_empty() {
                    self.set_activity("Enter a commit or ref to browse.");
                    cx.notify();
                    return;
                }
                self.pending_text_prompt = None;
                self.text_prompt_value.clear();
                self.tree_oid = value;
                let ShellState::Repository(repository) = &self.state else {
                    return;
                };
                let repository = repository.clone();
                self.navigate_to(RepositoryView::Tree, cx);
                self.tree_path.clear();
                self.tree_blob = None;
                self.load_tree(repository, cx);
            }
            TextPromptKind::HistorySearch => {
                self.pending_text_prompt = None;
                self.text_prompt_value.clear();
                self.history_search = value;
                self.history_list_state.reset(self.history_row_count());
                cx.notify();
            }
            TextPromptKind::HistoryReference => {
                if value.is_empty() {
                    self.set_activity("Enter a branch or tag name.");
                    cx.notify();
                    return;
                }
                self.pending_text_prompt = None;
                self.text_prompt_value.clear();
                let ShellState::Repository(repository) = &self.state else {
                    return;
                };
                self.change_history_reference(
                    HistoryReference::Named(value),
                    repository.clone(),
                    cx,
                );
            }
            TextPromptKind::RebaseOnto => {
                if value.is_empty() {
                    self.set_activity("Enter a rebase base (e.g. main).");
                    cx.notify();
                    return;
                }
                self.pending_text_prompt = None;
                self.text_prompt_value.clear();
                self.run_worktree_mutation(
                    format!("Rebase onto {value}"),
                    move |git, repository| git.start_rebase(repository, &value),
                    cx,
                );
            }
            TextPromptKind::MergeRevision => {
                if value.is_empty() {
                    self.set_activity("Enter a revision to merge.");
                    cx.notify();
                    return;
                }
                self.pending_text_prompt = None;
                self.text_prompt_value.clear();
                self.merge_branch_into_current(value, cx);
            }
            TextPromptKind::AutosquashTarget { squash } => {
                if value.is_empty() {
                    self.set_activity("Enter a commit to fold into (e.g. HEAD).");
                    cx.notify();
                    return;
                }
                if squash {
                    self.text_prompt_value.clear();
                    self.pending_text_prompt =
                        Some(TextPromptKind::AutosquashMessage { target: value });
                    self.pending_overlay_focus = Some(OverlayFocus::TextPrompt);
                    cx.notify();
                } else {
                    self.pending_text_prompt = None;
                    self.text_prompt_value.clear();
                    self.run_worktree_mutation(
                        format!("Fixup into {value}"),
                        move |git, repository| git.autosquash(repository, &value, None),
                        cx,
                    );
                }
            }
            TextPromptKind::AutosquashMessage { target } => {
                if value.is_empty() {
                    self.set_activity("Enter a squash message.");
                    cx.notify();
                    return;
                }
                self.pending_text_prompt = None;
                self.text_prompt_value.clear();
                self.run_worktree_mutation(
                    format!("Squash into {target}"),
                    move |git, repository| git.autosquash(repository, &target, Some(&value)),
                    cx,
                );
            }
            TextPromptKind::RewordSubject => {
                if value.is_empty() {
                    self.set_activity("Enter a commit subject.");
                    cx.notify();
                    return;
                }
                self.text_prompt_value.clear();
                self.pending_text_prompt = Some(TextPromptKind::RewordBody { subject: value });
                self.pending_overlay_focus = Some(OverlayFocus::TextPrompt);
                cx.notify();
            }
            TextPromptKind::RewordBody { subject } => {
                self.pending_text_prompt = None;
                self.text_prompt_value.clear();
                self.run_worktree_mutation(
                    "Reword last commit".to_owned(),
                    move |git, repository| {
                        git.commit(
                            repository,
                            &CommitRequest {
                                subject,
                                body: value,
                                amend: true,
                                sign_off: false,
                            },
                        )
                    },
                    cx,
                );
            }
            TextPromptKind::MergeToolPath => {
                self.pending_text_prompt = None;
                self.text_prompt_value.clear();
                self.run_merge_tool_for_path(if value.is_empty() { None } else { Some(value) }, cx);
            }
            TextPromptKind::CreateBookmarkFolder => {
                self.pending_text_prompt = None;
                self.text_prompt_value.clear();
                self.create_bookmark_folder(&value, cx);
            }
            TextPromptKind::RenameBookmarkFolder { id } => {
                self.pending_text_prompt = None;
                self.text_prompt_value.clear();
                self.rename_bookmark_folder(&id, &value, cx);
            }
        }
        cx.notify();
    }

    pub(crate) fn begin_choice_prompt(&mut self, kind: ChoicePromptKind, cx: &mut Context<Self>) {
        self.show_command_palette = false;
        self.show_quick_open = false;
        self.welcome_plus_menu_open = false;
        self.pending_text_prompt = None;
        self.text_prompt_value.clear();
        self.pending_choice_prompt = Some(kind);
        self.choice_prompt_query.clear();
        self.choice_prompt_selected = 0;
        self.pending_overlay_focus = Some(OverlayFocus::ChoicePrompt);
        cx.notify();
    }

    pub(crate) fn toggle_welcome_plus_menu(&mut self, cx: &mut Context<Self>) {
        self.show_command_palette = false;
        self.show_quick_open = false;
        self.pending_choice_prompt = None;
        self.choice_prompt_query.clear();
        self.choice_prompt_selected = 0;
        self.welcome_plus_menu_open = !self.welcome_plus_menu_open;
        cx.notify();
    }

    pub(crate) fn close_welcome_plus_menu(&mut self, cx: &mut Context<Self>) {
        if !self.welcome_plus_menu_open {
            return;
        }
        self.welcome_plus_menu_open = false;
        cx.notify();
    }

    pub(crate) fn add_repository_from_picker(&mut self, cx: &mut Context<Self>) {
        self.welcome_plus_menu_open = false;
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Choose a Git repository".into()),
        });
        let store = self.store.clone();
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(paths))) = receiver.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            let outcome = cx
                .background_spawn(async move { discover_and_record(&path, &store) })
                .await;
            let _ = this.update(cx, |app, cx| app.apply_open_outcome(outcome, cx));
        })
        .detach();
        cx.notify();
    }

    pub(crate) fn new_bookmark_group_from_menu(&mut self, cx: &mut Context<Self>) {
        self.welcome_plus_menu_open = false;
        self.begin_text_prompt(TextPromptKind::CreateBookmarkFolder, "", cx);
    }

    fn cancel_choice_prompt(&mut self, cx: &mut Context<Self>) {
        self.pending_choice_prompt = None;
        self.choice_prompt_query.clear();
        self.choice_prompt_selected = 0;
        cx.notify();
    }

    fn move_choice_prompt_selection(&mut self, delta: isize, cx: &mut Context<Self>) {
        let Some(kind) = self.pending_choice_prompt.as_ref() else {
            return;
        };
        let count = kind.filtered_options(&self.choice_prompt_query).len();
        if count == 0 {
            self.choice_prompt_selected = 0;
            cx.notify();
            return;
        }
        let current = self.choice_prompt_selected.min(count - 1);
        let next = match delta.cmp(&0) {
            std::cmp::Ordering::Less => current.saturating_sub(delta.unsigned_abs()),
            std::cmp::Ordering::Greater => (current + delta.unsigned_abs()).min(count - 1),
            std::cmp::Ordering::Equal => current,
        };
        self.choice_prompt_selected = next;
        cx.notify();
    }

    fn confirm_choice_prompt(&mut self, cx: &mut Context<Self>) {
        let Some(kind) = self.pending_choice_prompt.clone() else {
            return;
        };
        match kind {
            ChoicePromptKind::ConfirmMergePullRequest { number, method } => {
                self.pending_choice_prompt = None;
                self.choice_prompt_query.clear();
                self.choice_prompt_selected = 0;
                self.execute_merge_pull_request(number, method, cx);
            }
            ChoicePromptKind::SetMergeTool
            | ChoicePromptKind::MergePullRequest { .. }
            | ChoicePromptKind::BookmarkFolderActions { .. }
            | ChoicePromptKind::HistoryFilter
            | ChoicePromptKind::ResetMode { .. } => {
                let options = kind.filtered_options(&self.choice_prompt_query);
                let Some((_, label)) = options
                    .get(
                        self.choice_prompt_selected
                            .min(options.len().saturating_sub(1)),
                    )
                    .copied()
                else {
                    return;
                };
                self.select_choice_option(&kind, label, cx);
            }
        }
    }

    fn select_choice_option(
        &mut self,
        kind: &ChoicePromptKind,
        label: &'static str,
        cx: &mut Context<Self>,
    ) {
        match kind {
            ChoicePromptKind::SetMergeTool => {
                self.pending_choice_prompt = None;
                self.choice_prompt_query.clear();
                self.choice_prompt_selected = 0;
                let tool = label.to_owned();
                self.run_worktree_mutation(
                    format!("Set merge tool to {tool}"),
                    move |git, repository| git.set_merge_tool(repository, &tool),
                    cx,
                );
            }
            ChoicePromptKind::MergePullRequest { number } => {
                let Some((_, method)) = PR_MERGE_METHOD_CHOICES
                    .iter()
                    .find(|(choice_label, _)| *choice_label == label)
                else {
                    return;
                };
                self.pending_choice_prompt = Some(ChoicePromptKind::ConfirmMergePullRequest {
                    number: *number,
                    method: *method,
                });
                self.choice_prompt_query.clear();
                self.choice_prompt_selected = 0;
                self.pending_overlay_focus = Some(OverlayFocus::ChoicePrompt);
                cx.notify();
            }
            ChoicePromptKind::ConfirmMergePullRequest { number, method } => {
                self.pending_choice_prompt = None;
                self.choice_prompt_query.clear();
                self.choice_prompt_selected = 0;
                self.execute_merge_pull_request(*number, *method, cx);
            }
            ChoicePromptKind::BookmarkFolderActions { id } => {
                self.pending_choice_prompt = None;
                self.choice_prompt_query.clear();
                self.choice_prompt_selected = 0;
                match label {
                    "Rename…" => {
                        let current = self
                            .bookmark_folders
                            .iter()
                            .find(|folder| folder.id == *id)
                            .map(|folder| folder.name.clone())
                            .unwrap_or_default();
                        self.begin_text_prompt(
                            TextPromptKind::RenameBookmarkFolder { id: id.clone() },
                            current,
                            cx,
                        );
                    }
                    "Delete Group" => {
                        self.delete_bookmark_folder(id, cx);
                    }
                    _ => {}
                }
                cx.notify();
            }
            ChoicePromptKind::HistoryFilter => {
                self.pending_choice_prompt = None;
                self.choice_prompt_query.clear();
                self.choice_prompt_selected = 0;
                let ShellState::Repository(repository) = &self.state else {
                    return;
                };
                let repository = repository.clone();
                match label {
                    "Current branch" => {
                        self.change_history_reference(HistoryReference::Current, repository, cx);
                    }
                    "All refs" => {
                        self.change_history_reference(HistoryReference::All, repository, cx);
                    }
                    "Branch or tag…" => {
                        self.prompt_history_reference(cx);
                    }
                    "Search history…" => {
                        self.prompt_history_search(cx);
                    }
                    "Reveal HEAD" => {
                        self.reveal_history_head(cx);
                    }
                    "Copy selected OID" => {
                        self.copy_selected_history_oid(cx);
                    }
                    "New branch from commit…" => {
                        self.prompt_branch_from_selected(cx);
                    }
                    _ => {}
                }
            }
            ChoicePromptKind::ResetMode { oid } => {
                self.apply_reset_mode_choice(oid, label, cx);
            }
        }
    }

    fn apply_reset_mode_choice(&mut self, oid: &str, label: &str, cx: &mut Context<Self>) {
        self.pending_choice_prompt = None;
        self.choice_prompt_query.clear();
        self.choice_prompt_selected = 0;
        match label {
            "Soft" => self.run_reset_to(oid, ResetMode::Soft, cx),
            "Mixed (Keep Changes)" => self.run_reset_to(oid, ResetMode::Mixed, cx),
            "Hard" => {
                self.confirm_dialog = Some(AppConfirmDialog::HardReset {
                    oid: oid.to_owned(),
                });
                cx.notify();
            }
            _ => {}
        }
    }

    fn execute_merge_pull_request(
        &mut self,
        number: u64,
        method: git_domain::MergeMethod,
        cx: &mut Context<Self>,
    ) {
        let Some(repository) = self.pull_request_repository.clone() else {
            self.set_activity("Open pull requests before merging.");
            cx.notify();
            return;
        };
        let worker_repository = repository.clone();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    let keychain = MacKeychainStore;
                    let token = keychain
                        .read(&MacKeychainStore::github_key("default"))
                        .map_err(|_| HostingError::Network)?
                        .ok_or(HostingError::Authentication)?;
                    GitHubService::default().merge_pull_request(
                        &token,
                        &worker_repository,
                        number,
                        method,
                    )
                })
                .await;
            let _ = this.update(cx, |app, cx| {
                match result {
                    Ok(()) => {
                        app.set_activity(format!("Merged pull request #{number}."));
                        app.load_pull_requests(repository, cx);
                    }
                    Err(error) => app.set_activity(format!("Merge failed: {error:?}")),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn run_merge_tool_for_path(&mut self, path: Option<String>, cx: &mut Context<Self>) {
        let ShellState::Repository(repository) = &self.state else {
            self.set_activity("Open a repository before using the merge tool.");
            cx.notify();
            return;
        };
        let repository = repository.clone();
        let worker_repository = repository.clone();
        let path_arg = path.map(|path| GitPath(path.as_bytes().to_vec()));
        self.set_activity("Opening merge tool…");
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    let git = GitExecutable::discover().map_err(|error| error.to_string())?;
                    git.run_merge_tool(&worker_repository, None, path_arg.as_ref())
                        .map_err(|error| format!("{error:?}"))
                })
                .await;
            let _ = this.update(cx, |app, cx| match result {
                Ok(()) => {
                    app.set_activity("Merge tool finished.");
                    app.load_working_copy(repository, cx);
                }
                Err(error) => app.set_activity(format!("Merge tool failed: {error}")),
            });
        })
        .detach();
    }

    fn merge_branch_into_current(&mut self, branch: String, cx: &mut Context<Self>) {
        self.run_branch_command(
            format!("Merging {branch} into current"),
            move |git, repository| git.merge_branch(repository, &branch),
            cx,
        );
    }

    fn rebase_current_onto(&mut self, branch: String, cx: &mut Context<Self>) {
        self.run_branch_command(
            format!("Rebasing current onto {branch}"),
            move |git, repository| git.rebase_branch(repository, &branch),
            cx,
        );
    }

    fn prompt_rename_branch(&mut self, current: String, cx: &mut Context<Self>) {
        self.begin_text_prompt(TextPromptKind::BranchRename { current }, "", cx);
    }

    pub(crate) fn prompt_create_branch_from_ref(&mut self, start: String, cx: &mut Context<Self>) {
        self.begin_text_prompt(TextPromptKind::CreateBranch { start: Some(start) }, "", cx);
    }

    pub(crate) fn prompt_create_tag_from_ref(&mut self, start: String, cx: &mut Context<Self>) {
        self.begin_text_prompt(TextPromptKind::CreateTag { start }, "", cx);
    }

    pub(crate) fn copy_text_to_clipboard(&mut self, text: &str, cx: &mut Context<Self>) {
        cx.write_to_clipboard(ClipboardItem::new_string(text.to_owned()));
        self.set_activity(format!("Copied {text} to the clipboard."));
        cx.notify();
    }

    pub(crate) fn toggle_branch_pin(&mut self, branch: &str, cx: &mut Context<Self>) {
        if let Some(index) = self
            .branch_organization
            .pinned
            .iter()
            .position(|name| name == branch)
        {
            self.branch_organization.pinned.remove(index);
            self.set_activity(format!("Unpinned {branch}."));
        } else {
            self.branch_organization.pinned.push(branch.to_owned());
            self.branch_organization
                .archived
                .retain(|name| name != branch);
            self.set_activity(format!("Pinned {branch}."));
        }
        self.persist_branch_organization();
        cx.notify();
    }

    pub(crate) fn toggle_branch_archive(&mut self, branch: &str, cx: &mut Context<Self>) {
        if let Some(index) = self
            .branch_organization
            .archived
            .iter()
            .position(|name| name == branch)
        {
            self.branch_organization.archived.remove(index);
            self.set_activity(format!("Unarchived {branch}."));
        } else {
            self.branch_organization.archived.push(branch.to_owned());
            self.branch_organization
                .pinned
                .retain(|name| name != branch);
            self.set_activity(format!("Archived {branch}."));
        }
        self.persist_branch_organization();
        cx.notify();
    }

    fn persist_branch_organization(&mut self) {
        let ShellState::Repository(repository) = &self.state else {
            return;
        };
        if self
            .store
            .save_branch_organization(&repository.worktree_root, &self.branch_organization)
            .is_err()
        {
            self.set_activity("Branch organization could not be saved.");
        }
    }

    pub(crate) fn publish_branch(&mut self, branch: &str, cx: &mut Context<Self>) {
        let Some(remote) = self.default_remote() else {
            self.set_activity("No configured remote to publish to.");
            cx.notify();
            return;
        };
        let branch = branch.to_owned();
        self.run_network_command(
            format!("Publishing {branch} to {remote}"),
            vec![
                "push".into(),
                "--progress".into(),
                "--set-upstream".into(),
                remote.into(),
                branch.into(),
            ],
            cx,
        );
    }

    pub(crate) fn push_branch_to_destination(&mut self, destination: &str, cx: &mut Context<Self>) {
        self.open_push_dialog(Some(destination.to_owned()), cx);
    }

    pub(crate) fn set_branch_upstream(
        &mut self,
        branch: &str,
        upstream: &str,
        cx: &mut Context<Self>,
    ) {
        let branch = branch.to_owned();
        let upstream = upstream.to_owned();
        self.run_branch_command(
            format!("Tracking {upstream} from {branch}"),
            move |git, repository| git.set_branch_upstream(repository, &branch, &upstream),
            cx,
        );
    }

    pub(crate) fn unset_branch_upstream(&mut self, branch: &str, cx: &mut Context<Self>) {
        let branch = branch.to_owned();
        self.run_branch_command(
            format!("Stopping upstream tracking for {branch}"),
            move |git, repository| git.unset_branch_upstream(repository, &branch),
            cx,
        );
    }

    pub(crate) fn delete_tag(&mut self, tag: String, cx: &mut Context<Self>) {
        self.run_branch_command(
            format!("Deleting tag {tag}"),
            move |git, repository| git.delete_tag(repository, &tag),
            cx,
        );
    }

    pub(crate) fn request_remote_branch_delete(&mut self, branch: &str, cx: &mut Context<Self>) {
        let Some((remote, name)) = branch.split_once('/') else {
            self.set_activity(format!(
                "Remote branch {branch} is missing a remote prefix."
            ));
            cx.notify();
            return;
        };
        let remote = remote.to_owned();
        let name = name.to_owned();
        let label = format!("Deleting remote branch {branch}");
        self.run_network_command(
            label,
            vec![
                "push".into(),
                "--progress".into(),
                remote.into(),
                "--delete".into(),
                name.into(),
            ],
            cx,
        );
    }

    pub(crate) fn export_branch_archive(&mut self, reference: &str, cx: &mut Context<Self>) {
        let ShellState::Repository(repository) = &self.state else {
            return;
        };
        let repository = repository.clone();
        let reference = reference.to_owned();
        let default_name = format!("{}.zip", reference.replace('/', "-"));
        cx.spawn(async move |this, cx| {
            let destination = cx
                .background_spawn({
                    let default_name = default_name.clone();
                    async move {
                        let output = Command::new("osascript")
                            .args([
                                "-e",
                                &format!(
                                    "POSIX path of (choose file name with prompt \"Export archive\" default name \"{default_name}\")"
                                ),
                            ])
                            .output()
                            .ok()
                            .filter(|output| output.status.success())?;
                        let path = String::from_utf8_lossy(&output.stdout).trim().to_owned();
                        (!path.is_empty()).then(|| PathBuf::from(path))
                    }
                })
                .await;
            let Some(destination) = destination else {
                return;
            };
            let result = cx
                .background_spawn({
                    let repository = repository.clone();
                    let reference = reference.clone();
                    let destination = destination.clone();
                    async move {
                        let git = GitExecutable::discover().map_err(|error| error.to_string())?;
                        git.export_archive(&repository, &reference, &destination)
                            .map_err(|error| format!("{error:?}"))
                    }
                })
                .await;
            let _ = this.update(cx, |app, cx| {
                match result {
                    Ok(()) => {
                        app.set_activity(format!(
                            "Exported {reference} to {}.",
                            destination.display()
                        ));
                    }
                    Err(error) => app.set_activity(format!("Export failed: {error}")),
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn prompt_create_pull_request_from_branch(
        &mut self,
        head: &str,
        cx: &mut Context<Self>,
    ) {
        if self.pull_request_repository.is_none()
            && let Some(hosted) = self.hosted_repositories.first().cloned()
        {
            self.pull_request_repository = Some(hosted);
        }
        let Some(repository) = self.pull_request_repository.clone() else {
            self.set_activity("Connect a GitHub account in Settings first.");
            cx.notify();
            return;
        };
        let head = head.to_owned();
        let base = self
            .default_remote()
            .map_or_else(|| "main".into(), |remote| format!("{remote}/main"));
        cx.spawn(async move |this, cx| {
            let fields = cx
                .background_spawn({
                    let head = head.clone();
                    let base = base.clone();
                    async move {
                        let script = format!(
                            r#"set title_text to text returned of (display dialog "Pull request title" default answer "")
set body_text to text returned of (display dialog "Pull request description" default answer "")
set head_text to text returned of (display dialog "Head branch" default answer "{head}")
set base_text to text returned of (display dialog "Base branch" default answer "{base}")
return title_text & linefeed & body_text & linefeed & head_text & linefeed & base_text"#
                        );
                        let output = Command::new("osascript")
                            .args(["-e", &script])
                            .output()
                            .ok()
                            .filter(|output| output.status.success())?;
                        let fields = String::from_utf8_lossy(&output.stdout)
                            .lines()
                            .map(str::trim_end)
                            .map(ToOwned::to_owned)
                            .collect::<Vec<_>>();
                        (fields.len() == 4).then_some(fields)
                    }
                })
                .await;
            let Some(fields) = fields else {
                return;
            };
            if fields.iter().any(String::is_empty) {
                return;
            }
            let worker_repository = repository.clone();
            let result = cx
                .background_spawn(async move {
                    let keychain = MacKeychainStore;
                    let token = keychain
                        .read(&MacKeychainStore::github_key("default"))
                        .map_err(|_| HostingError::Network)?
                        .ok_or(HostingError::Authentication)?;
                    GitHubService::default().create_pull_request(
                        &token,
                        &worker_repository,
                        &fields[0],
                        &fields[1],
                        &fields[2],
                        &fields[3],
                    )
                })
                .await;
            let _ = this.update(cx, |app, cx| {
                match result {
                    Ok(request) => {
                        app.set_activity(format!("Created pull request #{}.", request.number));
                        app.load_pull_requests(repository, cx);
                    }
                    Err(error) => app.set_activity(format!("Create pull request failed: {error:?}")),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn prompt_branch_from_selected(&mut self, cx: &mut Context<Self>) {
        let Some(oid) = self
            .selected_history
            .and_then(|index| self.history.get(index))
            .map(|commit| commit.oid.clone())
        else {
            self.set_activity("Select a history commit first.");
            cx.notify();
            return;
        };
        self.begin_text_prompt(TextPromptKind::CreateBranch { start: Some(oid) }, "", cx);
    }

    fn prompt_file_history(&mut self, cx: &mut Context<Self>) {
        self.begin_text_prompt(TextPromptKind::FileHistoryPath, "", cx);
    }

    fn prompt_blame(&mut self, cx: &mut Context<Self>) {
        self.begin_text_prompt(TextPromptKind::BlamePath, "", cx);
    }

    fn prompt_compare_refs(&mut self, cx: &mut Context<Self>) {
        self.begin_text_prompt(TextPromptKind::CompareFrom, "HEAD", cx);
    }

    fn move_history_selection(&mut self, delta: isize, cx: &mut Context<Self>) {
        if self.repository_view != RepositoryView::History || self.history.is_empty() {
            return;
        }
        let current = self.selected_history.unwrap_or(0);
        let index = if delta.is_negative() {
            current.saturating_sub(delta.unsigned_abs())
        } else {
            current
                .saturating_add(delta.unsigned_abs())
                .min(self.history.len() - 1)
        };
        let ShellState::Repository(repository) = &self.state else {
            return;
        };
        self.select_history_commit(index, repository.clone(), cx);
    }

    pub(crate) fn show_history(&mut self, repository: WorktreeRepository, cx: &mut Context<Self>) {
        self.navigate_to(RepositoryView::History, cx);
        if self.history.is_empty() {
            self.load_history(repository, None, cx);
        }
        cx.notify();
    }

    fn show_commit_detail(
        &mut self,
        repository: &WorktreeRepository,
        index: usize,
        cx: &mut Context<Self>,
    ) {
        let Some(oid) = self.history.get(index).map(|commit| commit.oid.clone()) else {
            return;
        };
        self.navigate_to(RepositoryView::CommitDetail, cx);
        self.history_detail_mode = HistoryDetailMode::Changeset;
        self.tree_oid = oid;
        self.tree.clear();
        self.tree_path.clear();
        self.tree_blob = None;
        self.tree_blob_path = None;
        if self.history_paths.is_empty() {
            self.select_history_commit(index, repository.clone(), cx);
        }
        cx.notify();
    }

    fn toggle_history_detail_mode(
        &mut self,
        mode: HistoryDetailMode,
        repository: WorktreeRepository,
        cx: &mut Context<Self>,
    ) {
        if self.history_detail_mode == mode {
            return;
        }
        self.history_detail_mode = mode;
        self.tree.clear();
        self.tree_blob = None;
        if mode == HistoryDetailMode::Tree {
            self.load_tree(repository, cx);
        }
        cx.notify();
    }

    fn change_history_reference(
        &mut self,
        reference: HistoryReference,
        repository: WorktreeRepository,
        cx: &mut Context<Self>,
    ) {
        self.history_reference = reference;
        self.reset_history();
        self.load_history(repository, None, cx);
    }

    fn reset_history(&mut self) {
        self.history.clear();
        self.history_rows.clear();
        self.history_state = GraphState::default();
        self.history_next = None;
        self.history_decorations.clear();
        self.selected_history = None;
        self.history_list_state.reset(0);
        self.history_paths.clear();
        self.history_diff = None;
        self.history_selection_token = self.history_selection_token.wrapping_add(1);
        self.history_load_token = self.history_load_token.wrapping_add(1);
    }

    fn load_history(
        &mut self,
        repository: WorktreeRepository,
        before: Option<String>,
        cx: &mut Context<Self>,
    ) {
        let root = repository.worktree_root.clone();
        let worker_repository = repository.clone();
        let select_repository = repository;
        let reference = self.history_reference.clone();
        let load_token = self.history_load_token;
        let initial_page = before.is_none();
        self.set_activity("Loading history…");
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    let git = GitExecutable::discover().map_err(|error| error.to_string())?;
                    let page = git
                        .history_page(
                            &worker_repository,
                            &HistoryRequest {
                                reference,
                                before,
                                limit: 100,
                            },
                        )
                        .map_err(|error| format!("{error:?}"))?;
                    let decorations = git
                        .ref_decorations(&worker_repository)
                        .map_err(|error| format!("{error:?}"))?;
                    Ok::<_, String>((page, decorations))
                })
                .await;
            let _ = this.update(cx, |app, cx| {
                if !matches!(&app.state, ShellState::Repository(current) if current.worktree_root == root)
                    || app.history_load_token != load_token
                {
                    return;
                }
                match result {
                    Ok((HistoryPage { commits, next_before }, decorations)) => {
                        let rows = layout_history_graph(&commits, &mut app.history_state);
                        app.history.extend(commits);
                        app.history_rows.extend(rows);
                        app.history_next = next_before;
                        app.history_decorations = decorations;
                        app.set_activity(format!("Loaded {} history commits.", app.history.len()));
                        app.history_list_state.reset(app.history_row_count());
                        let reveal_index = app
                            .history_reveal_oid
                            .take()
                            .and_then(|oid| app.history.iter().position(|commit| commit.oid == oid));
                        let select_index = reveal_index.or_else(|| {
                            if initial_page && app.selected_history.is_none() && !app.history.is_empty()
                            {
                                Some(0)
                            } else {
                                None
                            }
                        });
                        if let Some(index) = select_index {
                            app.select_history_commit(index, select_repository, cx);
                        }
                    }
                    Err(error) => app.set_activity(format!("History load failed: {error}")),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn show_reflog(&mut self, repository: WorktreeRepository, cx: &mut Context<Self>) {
        self.navigate_to(RepositoryView::Reflog, cx);
        if self.reflog.is_empty() {
            self.load_reflog(repository, cx);
        }
        cx.notify();
    }

    fn load_reflog(&mut self, repository: WorktreeRepository, cx: &mut Context<Self>) {
        let root = repository.worktree_root.clone();
        let load_token = self.reflog_load_token;
        self.set_activity("Loading reflog…");
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    let git = GitExecutable::discover().map_err(|error| error.to_string())?;
                    let entries = git
                        .reflog(
                            &repository,
                            &ReflogRequest {
                                reference: None,
                                limit: 200,
                            },
                        )
                        .map_err(|error| format!("{error:?}"))?;
                    Ok::<_, String>(entries)
                })
                .await;
            let _ = this.update(cx, |app, cx| {
                if !matches!(&app.state, ShellState::Repository(current) if current.worktree_root == root)
                    || app.reflog_load_token != load_token
                {
                    return;
                }
                match result {
                    Ok(entries) => {
                        app.reflog = entries;
                        app.selected_reflog = None;
                        app.set_activity(format!("Loaded {} reflog entries.", app.reflog.len()));
                    }
                    Err(error) => app.set_activity(format!("Reflog load failed: {error}")),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn move_reflog_selection(&mut self, delta: isize, cx: &mut Context<Self>) {
        if self.repository_view != RepositoryView::Reflog || self.reflog.is_empty() {
            return;
        }
        let current = self.selected_reflog.unwrap_or(0);
        let index = if delta.is_negative() {
            current.saturating_sub(delta.unsigned_abs())
        } else {
            current
                .saturating_add(delta.unsigned_abs())
                .min(self.reflog.len() - 1)
        };
        self.selected_reflog = Some(index);
        cx.notify();
    }

    fn prompt_restore_branch_from_reflog(cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let name = cx
                .background_spawn(async {
                    Command::new("osascript")
                        .args(["-e", "text returned of (display dialog \"Restore lost branch from reflog\" default answer \"\")"])
                        .output()
                        .ok()
                        .filter(|output| output.status.success())
                        .map(|output| String::from_utf8_lossy(&output.stdout).trim_end().to_owned())
                        .filter(|name| !name.is_empty())
                })
                .await;
            let _ = this.update(cx, |app, cx| {
                if let Some(name) = name {
                    app.restore_branch_from_reflog(name, cx);
                }
            });
        })
        .detach();
    }

    fn restore_branch_from_reflog(&mut self, branch: String, cx: &mut Context<Self>) {
        if self.mutation_in_flight {
            return;
        }
        let Some(selected) = self.selected_reflog else {
            return;
        };
        let Some(entry) = self.reflog.get(selected) else {
            return;
        };
        let oid = String::from_utf8_lossy(&entry.new_oid).into_owned();
        let ShellState::Repository(repository) = &self.state else {
            return;
        };
        let repository = repository.clone();
        let worker_repository = repository.clone();
        let worker_branch = branch.clone();
        self.mutation_in_flight = true;
        self.set_activity(format!("Restoring branch {branch}…"));
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    let git = GitExecutable::discover().map_err(|error| error.to_string())?;
                    git.restore_branch_from_reflog(&worker_repository, &worker_branch, &oid)
                        .map_err(|error| format!("{error:?}"))
                })
                .await;
            let _ = this.update(cx, |app, cx| {
                app.mutation_in_flight = false;
                match result {
                    Ok(()) => {
                        app.set_activity(format!("Restored branch {branch}."));
                        app.load_working_copy(repository.clone(), cx);
                        Self::load_refs(repository, cx);
                    }
                    Err(error) => {
                        app.set_activity(git_failure_message(
                            &format!("Restore branch {branch}"),
                            &error,
                        ));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn load_file_history(&mut self, repository: WorktreeRepository, cx: &mut Context<Self>) {
        let root = repository.worktree_root.clone();
        let load_token = self.file_history_load_token;
        let path = self.file_history_path.clone();
        self.set_activity(format!("Loading history for {path}…"));
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    let git = GitExecutable::discover().map_err(|error| error.to_string())?;
                    let commits = git
                        .file_history(
                            &repository,
                            &FileHistoryRequest {
                                path: GitPath(path.as_bytes().to_vec()),
                                limit: 100,
                            },
                        )
                        .map_err(|error| format!("{error:?}"))?;
                    Ok::<_, String>(commits)
                })
                .await;
            let _ = this.update(cx, |app, cx| {
                if !matches!(&app.state, ShellState::Repository(current) if current.worktree_root == root)
                    || app.file_history_load_token != load_token
                {
                    return;
                }
                match result {
                    Ok(commits) => {
                        app.file_history = commits;
                        app.set_activity(format!(
                            "Loaded {} commits for {}.",
                            app.file_history.len(),
                            app.file_history_path
                        ));
                    }
                    Err(error) => app.set_activity(format!("File history failed: {error}")),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn load_blame(&mut self, repository: WorktreeRepository, cx: &mut Context<Self>) {
        let root = repository.worktree_root.clone();
        let load_token = self.blame_load_token;
        let path = self.blame_path.clone();
        self.set_activity(format!("Loading blame for {path}…"));
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    let git = GitExecutable::discover().map_err(|error| error.to_string())?;
                    let lines = git
                        .blame(&repository, &GitPath(path.as_bytes().to_vec()))
                        .map_err(|error| format!("{error:?}"))?;
                    Ok::<_, String>(lines)
                })
                .await;
            let _ = this.update(cx, |app, cx| {
                if !matches!(&app.state, ShellState::Repository(current) if current.worktree_root == root)
                    || app.blame_load_token != load_token
                {
                    return;
                }
                match result {
                    Ok(lines) => {
                        app.blame = lines;
                        app.set_activity(format!(
                            "Loaded blame for {} ({} lines).",
                            app.blame_path,
                            app.blame.len()
                        ));
                    }
                    Err(error) => app.set_activity(format!("Blame failed: {error}")),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn load_compare(&mut self, repository: WorktreeRepository, cx: &mut Context<Self>) {
        let root = repository.worktree_root.clone();
        let load_token = self.compare_load_token;
        let left = self.compare_left.clone();
        let right = self.compare_right.clone();
        self.set_activity(format!("Comparing {left}…{right}…"));
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    let git = GitExecutable::discover().map_err(|error| error.to_string())?;
                    let loaded = git
                        .diff_refs(&repository, &left, &right)
                        .map_err(|error| format!("{error:?}"))?;
                    Ok::<_, String>(loaded)
                })
                .await;
            let _ = this.update(cx, |app, cx| {
                if !matches!(&app.state, ShellState::Repository(current) if current.worktree_root == root)
                    || app.compare_load_token != load_token
                {
                    return;
                }
                match result {
                    Ok(loaded) => {
                        app.compare_diff = Some(loaded);
                        app.set_activity(format!(
                            "Compared {}…{} ({} file(s)).",
                            app.compare_left,
                            app.compare_right,
                            app.compare_diff
                                .as_ref()
                                .map_or(0, |loaded| loaded.diff.files.len())
                        ));
                    }
                    Err(error) => app.set_activity(format!("Compare failed: {error}")),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn prompt_browse_tree(&mut self, cx: &mut Context<Self>) {
        self.begin_text_prompt(TextPromptKind::BrowseTree, "HEAD", cx);
    }

    fn load_tree(&mut self, repository: WorktreeRepository, cx: &mut Context<Self>) {
        let root = repository.worktree_root.clone();
        let load_token = self.tree_load_token;
        let oid = self.tree_oid.clone();
        let path = joined_tree_path(&self.tree_path);
        self.set_activity(format!("Loading tree {}…", self.tree_path_label()));
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    let git = GitExecutable::discover().map_err(|error| error.to_string())?;
                    let entries = git
                        .tree_entries(&repository, &oid, &path)
                        .map_err(|error| format!("{error:?}"))?;
                    Ok::<_, String>(entries)
                })
                .await;
            let _ = this.update(cx, |app, cx| {
                if !matches!(&app.state, ShellState::Repository(current) if current.worktree_root == root)
                    || app.tree_load_token != load_token
                {
                    return;
                }
                match result {
                    Ok(entries) => {
                        app.tree = entries;
                        app.set_activity(format!("Listed {} entries.", app.tree.len()));
                    }
                    Err(error) => app.set_activity(format!("Tree load failed: {error}")),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn tree_path_label(&self) -> String {
        joined_tree_path(&self.tree_path)
            .0
            .split(|byte| *byte == b'/')
            .filter(|segment| !segment.is_empty())
            .map(|segment| String::from_utf8_lossy(segment).into_owned())
            .collect::<Vec<_>>()
            .join("/")
    }

    fn select_tree_entry(
        &mut self,
        entry: &TreeEntry,
        repository: WorktreeRepository,
        cx: &mut Context<Self>,
    ) {
        let name = entry.name.clone();
        match entry.kind {
            TreeEntryKind::Tree => {
                self.tree_path.push(name);
                self.tree_blob = None;
                self.tree_blob_path = None;
                self.load_tree(repository, cx);
            }
            TreeEntryKind::Blob => {
                let mut full_path = joined_tree_path(&self.tree_path);
                if !full_path.0.is_empty() {
                    full_path.0.push(b'/');
                }
                full_path.0.extend_from_slice(&name.0);
                self.tree_blob_path = Some(full_path.clone());
                let root = repository.worktree_root.clone();
                let load_token = self.tree_load_token;
                let oid = self.tree_oid.clone();
                self.set_activity(format!("Reading {}", String::from_utf8_lossy(&full_path.0)));
                cx.spawn(async move |this, cx| {
                    let result = cx
                        .background_spawn(async move {
                            let git = GitExecutable::discover().map_err(|error| error.to_string())?;
                            let bytes = git
                                .file_at_revision(&repository, &oid, &full_path)
                                .map_err(|error| format!("{error:?}"))?;
                            Ok::<_, String>(bytes)
                        })
                        .await;
                    let _ = this.update(cx, |app, cx| {
                        if !matches!(&app.state, ShellState::Repository(current) if current.worktree_root == root)
                            || app.tree_load_token != load_token
                        {
                            return;
                        }
                        match result {
                            Ok(bytes) => {
                                app.tree_blob = Some(bytes);
                                app.set_activity(String::new());
                            }
                            Err(error) => app.set_activity(format!("Blob read failed: {error}")),
                        }
                        cx.notify();
                    });
                })
                .detach();
            }
            TreeEntryKind::Commit => {
                cx.notify();
            }
        }
    }

    fn back_tree_level(&mut self, repository: WorktreeRepository, cx: &mut Context<Self>) {
        if self.tree_path.pop().is_some() {
            self.tree_blob = None;
            self.tree_blob_path = None;
            self.load_tree(repository, cx);
        }
        cx.notify();
    }

    fn export_selected_blob(cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let destination = cx
                .background_spawn(async {
                    Command::new("osascript")
                        .args([
                            "-e",
                            "POSIX path of (choose folder with prompt \"Export file to folder\")",
                        ])
                        .output()
                        .ok()
                        .filter(|output| output.status.success())
                        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
                        .filter(|path| !path.is_empty())
                })
                .await;
            let _ = this.update(cx, |app, cx| {
                let Some(destination) = destination else {
                    return;
                };
                let Some(blob) = app.tree_blob.as_ref() else {
                    app.set_activity("Select a file in the tree before exporting.");
                    cx.notify();
                    return;
                };
                let Some(file_name) = app.tree_blob_path.as_ref().map(|path| {
                    let name = path
                        .0
                        .iter()
                        .rposition(|byte| *byte == b'/')
                        .map_or(&path.0[..], |separator| &path.0[separator + 1..]);
                    String::from_utf8_lossy(name).into_owned()
                }) else {
                    app.set_activity("The selected blob has no file name.");
                    cx.notify();
                    return;
                };
                let path = std::path::Path::new(&destination).join(file_name);
                match std::fs::write(&path, blob) {
                    Ok(()) => {
                        app.set_activity(format!("Exported to {}.", path.display()));
                    }
                    Err(error) => {
                        app.set_activity(format!("Export failed: {error}"));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn show_worktrees(&mut self, repository: WorktreeRepository, cx: &mut Context<Self>) {
        self.navigate_to(RepositoryView::Worktrees, cx);
        if self.worktrees.is_empty() {
            self.load_worktrees(repository, cx);
        }
        cx.notify();
    }

    fn load_worktrees(&mut self, repository: WorktreeRepository, cx: &mut Context<Self>) {
        let root = repository.worktree_root.clone();
        let load_token = self.worktrees_load_token;
        self.set_activity("Loading worktrees…");
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    let git = GitExecutable::discover().map_err(|error| error.to_string())?;
                    let entries = git
                        .worktree_list(&repository)
                        .map_err(|error| format!("{error:?}"))?;
                    Ok::<_, String>(entries)
                })
                .await;
            let _ = this.update(cx, |app, cx| {
                if !matches!(&app.state, ShellState::Repository(current) if current.worktree_root == root)
                    || app.worktrees_load_token != load_token
                {
                    return;
                }
                match result {
                    Ok(entries) => {
                        app.worktrees = entries;
                        app.set_activity(format!("Loaded {} worktree(s).", app.worktrees.len()));
                    }
                    Err(error) => app.set_activity(format!("Worktree load failed: {error}")),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn run_worktree_mutation(
        &mut self,
        label: String,
        command: impl FnOnce(&GitExecutable, &WorktreeRepository) -> Result<(), GitStatusError>
        + Send
        + 'static,
        cx: &mut Context<Self>,
    ) {
        if self.mutation_in_flight {
            return;
        }
        let ShellState::Repository(repository) = &self.state else {
            return;
        };
        let repository = repository.clone();
        let worker_repository = repository.clone();
        self.mutation_in_flight = true;
        self.set_activity(format!("{label}…"));
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    let git = GitExecutable::discover().map_err(|error| error.to_string())?;
                    command(&git, &worker_repository).map_err(|error| format!("{error:?}"))
                })
                .await;
            let _ = this.update(cx, |app, cx| {
                app.mutation_in_flight = false;
                match result {
                    Ok(()) => {
                        app.set_activity(format!("{label} complete."));
                        app.load_working_copy(repository.clone(), cx);
                        Self::load_refs(repository.clone(), cx);
                        if app.repository_view == RepositoryView::Worktrees {
                            app.worktrees_load_token = app.worktrees_load_token.wrapping_add(1);
                            app.load_worktrees(repository.clone(), cx);
                        }
                        if app.repository_view == RepositoryView::Submodules {
                            app.submodules_load_token = app.submodules_load_token.wrapping_add(1);
                            app.load_submodules(repository.clone(), cx);
                        }
                        if app.repository_view == RepositoryView::Rebase {
                            app.reload_rebase_plan(&repository, cx);
                        }
                    }
                    Err(error) => app.set_activity(git_failure_message(&label, &error)),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn prompt_add_worktree(cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let path = cx
                .background_spawn(async {
                    Command::new("osascript")
                        .args(["-e", "text returned of (display dialog \"New worktree path\" default answer \"\")"])
                        .output()
                        .ok()
                        .filter(|output| output.status.success())
                        .map(|output| String::from_utf8_lossy(&output.stdout).trim_end().to_owned())
                        .filter(|path| !path.is_empty())
                })
                .await;
            let branch = cx
                .background_spawn(async {
                    Command::new("osascript")
                        .args(["-e", "text returned of (display dialog \"New branch in worktree\" default answer \"\")"])
                        .output()
                        .ok()
                        .filter(|output| output.status.success())
                        .map(|output| String::from_utf8_lossy(&output.stdout).trim_end().to_owned())
                        .filter(|branch| !branch.is_empty())
                })
                .await;
            let _ = this.update(cx, |app, cx| {
                let (Some(path), Some(branch)) = (path, branch) else {
                    return;
                };
                let path_arg = GitPath(path.as_bytes().to_vec());
                app.run_worktree_mutation(
                    format!("Add worktree at {path}"),
                    move |git, repository| git.add_worktree(repository, &path_arg, &branch),
                    cx,
                );
            });
        })
        .detach();
    }

    fn prompt_remove_worktree(cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let path = cx
                .background_spawn(async {
                    Command::new("osascript")
                        .args(["-e", "text returned of (display dialog \"Remove worktree at path\" default answer \"\")"])
                        .output()
                        .ok()
                        .filter(|output| output.status.success())
                        .map(|output| String::from_utf8_lossy(&output.stdout).trim_end().to_owned())
                        .filter(|path| !path.is_empty())
                })
                .await;
            let _ = this.update(cx, |app, cx| {
                if let Some(path) = path {
                    let path_arg = GitPath(path.as_bytes().to_vec());
                    app.run_worktree_mutation(
                        format!("Remove worktree at {path}"),
                        move |git, repository| git.remove_worktree(repository, &path_arg, false),
                        cx,
                    );
                }
            });
        })
        .detach();
    }

    fn show_submodules(&mut self, repository: WorktreeRepository, cx: &mut Context<Self>) {
        self.navigate_to(RepositoryView::Submodules, cx);
        if self.submodules.is_empty() {
            self.load_submodules(repository, cx);
        }
    }

    fn load_submodules(&mut self, repository: WorktreeRepository, cx: &mut Context<Self>) {
        let root = repository.worktree_root.clone();
        let load_token = self.submodules_load_token;
        self.set_activity("Loading submodules…");
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    let git = GitExecutable::discover().map_err(|error| error.to_string())?;
                    git.submodule_list(&repository)
                        .map_err(|error| format!("{error:?}"))
                })
                .await;
            let _ = this.update(cx, |app, cx| {
                if !matches!(
                    &app.state,
                    ShellState::Repository(current) if current.worktree_root == root
                ) || app.submodules_load_token != load_token
                {
                    return;
                }
                match result {
                    Ok(entries) => {
                        app.submodules = entries;
                        app.set_activity(format!("Loaded {} submodule(s).", app.submodules.len()));
                    }
                    Err(error) => app.set_activity(format!("Submodule load failed: {error}")),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn show_lfs(&mut self, repository: WorktreeRepository, cx: &mut Context<Self>) {
        self.navigate_to(RepositoryView::Lfs, cx);
        self.load_lfs(repository, cx);
    }

    fn load_lfs(&mut self, repository: WorktreeRepository, cx: &mut Context<Self>) {
        let root = repository.worktree_root.clone();
        self.lfs_load_token = self.lfs_load_token.wrapping_add(1);
        let load_token = self.lfs_load_token;
        self.set_activity("Loading Git LFS status…");
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    let git = GitExecutable::discover().map_err(|error| error.to_string())?;
                    git.lfs_status(&repository)
                        .map_err(|error| format!("{error:?}"))
                })
                .await;
            let _ = this.update(cx, |app, cx| {
                if !matches!(
                    &app.state,
                    ShellState::Repository(current) if current.worktree_root == root
                ) || app.lfs_load_token != load_token
                {
                    return;
                }
                match result {
                    Ok(entries) => {
                        app.lfs = entries;
                        app.set_activity(format!("Loaded {} Git LFS change(s).", app.lfs.len()));
                    }
                    Err(error) => app.set_activity(git_failure_message("Git LFS status", &error)),
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn load_github_account(&mut self, cx: &mut Context<Self>) {
        self.github_load_token = self.github_load_token.wrapping_add(1);
        let load_token = self.github_load_token;
        self.service_auth_state = git_domain::ServiceAuthState::Loading;
        self.set_activity("Loading GitHub account…");
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async {
                    let keychain = MacKeychainStore;
                    let key = MacKeychainStore::github_key("default");
                    let Some(token) = keychain.read(&key).map_err(|_| HostingError::Network)?
                    else {
                        return Ok::<_, HostingError>(None);
                    };
                    let service = GitHubService::default();
                    let account = service.authenticate(&token)?;
                    let repositories = service.repositories(&token)?;
                    Ok(Some((account, repositories)))
                })
                .await;
            let _ = this.update(cx, |app, cx| {
                if app.github_load_token != load_token {
                    return;
                }
                match result {
                    Ok(Some((account, repositories))) => {
                        app.service_auth_state = git_domain::ServiceAuthState::Connected;
                        app.service_account = Some(account);
                        app.hosted_repositories = repositories;
                        app.set_activity(format!(
                            "Loaded {} GitHub repositories.",
                            app.hosted_repositories.len()
                        ));
                    }
                    Ok(None) => {
                        app.service_auth_state = git_domain::ServiceAuthState::SignedOut;
                        app.service_account = None;
                        app.hosted_repositories.clear();
                        app.set_activity("Connect a GitHub account in Settings.");
                    }
                    Err(error) => {
                        app.service_auth_state = service_auth_state(&error);
                        app.set_activity(format!("GitHub account failed: {error:?}"));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn prompt_connect_github(cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let token = cx
                .background_spawn(async {
                    Command::new("osascript")
                        .args(["-e", "text returned of (display dialog \"GitHub personal access token\" default answer \"\" with hidden answer true)"])
                        .output()
                        .ok()
                        .filter(|output| output.status.success())
                        .map(|output| String::from_utf8_lossy(&output.stdout).trim_end().to_owned())
                        .filter(|token| !token.is_empty())
                })
                .await;
            let Some(token) = token else {
                return;
            };
            let result = cx
                .background_spawn(async move {
                    let keychain = MacKeychainStore;
                    let key = MacKeychainStore::github_key("default");
                    keychain
                        .write(&key, &token)
                        .map_err(|_| HostingError::Network)?;
                    let service = GitHubService::default();
                    let account = service.authenticate(&token)?;
                    let repositories = service.repositories(&token)?;
                    Ok::<_, HostingError>((account, repositories))
                })
                .await;
            let _ = this.update(cx, |app, cx| {
                match result {
                    Ok((account, repositories)) => {
                        app.service_auth_state = git_domain::ServiceAuthState::Connected;
                        app.service_account = Some(account);
                        app.hosted_repositories = repositories;
                        app.set_activity(format!(
                            "Connected GitHub and loaded {} repositories.",
                            app.hosted_repositories.len()
                        ));
                    }
                    Err(error) => {
                        app.service_auth_state = service_auth_state(&error);
                        app.set_activity(format!("GitHub connection failed: {error:?}"));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn sign_out_github(&mut self, cx: &mut Context<Self>) {
        let result = MacKeychainStore.delete(&MacKeychainStore::github_key("default"));
        self.service_auth_state = git_domain::ServiceAuthState::SignedOut;
        self.service_account = None;
        self.hosted_repositories.clear();
        self.set_activity(if result.is_ok() {
            "GitHub account disconnected."
        } else {
            "GitHub account cleared from this session; Keychain removal failed."
        });
        cx.notify();
    }

    fn open_pull_requests(&mut self, cx: &mut Context<Self>) {
        if let Some(repository) = self.pull_request_repository.clone() {
            self.show_pull_requests(repository, cx);
            return;
        }
        if let Some(repository) = self.hosted_repositories.first().cloned() {
            self.show_pull_requests(repository, cx);
            return;
        }
        self.set_activity("Connect a GitHub account in Settings first.");
        self.navigate_to(RepositoryView::Settings, cx);
    }

    fn show_pull_requests(
        &mut self,
        repository: git_domain::HostedRepository,
        cx: &mut Context<Self>,
    ) {
        self.pull_request_repository = Some(repository.clone());
        self.pull_requests.clear();
        self.selected_pull_request = None;
        self.pull_request_detail = None;
        self.navigate_to(RepositoryView::PullRequests, cx);
        self.load_pull_requests(repository, cx);
    }

    fn load_pull_requests(
        &mut self,
        repository: git_domain::HostedRepository,
        cx: &mut Context<Self>,
    ) {
        self.pull_requests_load_token = self.pull_requests_load_token.wrapping_add(1);
        let load_token = self.pull_requests_load_token;
        self.set_activity("Loading pull requests…");
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    let keychain = MacKeychainStore;
                    let token = keychain
                        .read(&MacKeychainStore::github_key("default"))
                        .map_err(|_| HostingError::Network)?
                        .ok_or(HostingError::Authentication)?;
                    GitHubService::default().pull_requests(&token, &repository)
                })
                .await;
            let _ = this.update(cx, |app, cx| {
                if app.pull_requests_load_token != load_token {
                    return;
                }
                match result {
                    Ok(entries) => {
                        app.pull_requests = entries;
                        app.selected_pull_request = None;
                        app.pull_request_detail = None;
                        app.set_activity(format!(
                            "Loaded {} open pull request(s).",
                            app.pull_requests.len()
                        ));
                    }
                    Err(error) => app.set_activity(format!("Pull request load failed: {error:?}")),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn select_pull_request(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(request) = self.pull_requests.get(index) else {
            return;
        };
        let Some(repository) = self.pull_request_repository.clone() else {
            return;
        };
        self.selected_pull_request = Some(index);
        self.pull_request_detail = None;
        self.pull_request_detail_token = self.pull_request_detail_token.wrapping_add(1);
        let detail_token = self.pull_request_detail_token;
        let number = request.number;
        self.set_activity(format!("Loading pull request #{number}…"));
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    let keychain = MacKeychainStore;
                    let token = keychain
                        .read(&MacKeychainStore::github_key("default"))
                        .map_err(|_| HostingError::Network)?
                        .ok_or(HostingError::Authentication)?;
                    GitHubService::default().pull_request_detail(&token, &repository, number)
                })
                .await;
            let _ = this.update(cx, |app, cx| {
                if app.pull_request_detail_token != detail_token {
                    return;
                }
                match result {
                    Ok(detail) => {
                        app.pull_request_detail = Some(detail);
                        app.set_activity(format!("Loaded pull request #{number}."));
                    }
                    Err(error) => {
                        app.set_activity(format!("Pull request detail failed: {error:?}"));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    #[allow(clippy::too_many_lines)]
    fn prompt_create_pull_request(&mut self, cx: &mut Context<Self>) {
        let Some(repository) = self.pull_request_repository.clone() else {
            self.set_activity("Connect a GitHub account in Settings first.");
            cx.notify();
            return;
        };
        cx.spawn(async move |this, cx| {
            let fields = cx
                .background_spawn(async {
                    let output = Command::new("osascript")
                        .args(["-e", CREATE_PULL_REQUEST_SCRIPT])
                        .output()
                        .ok()
                        .filter(|output| output.status.success())?;
                    let fields = String::from_utf8_lossy(&output.stdout)
                        .lines()
                        .map(str::trim_end)
                        .map(ToOwned::to_owned)
                        .collect::<Vec<_>>();
                    (fields.len() == 4).then_some(fields)
                })
                .await;
            let Some(fields) = fields else {
                return;
            };
            if fields.iter().any(String::is_empty) {
                return;
            }
            let worker_repository = repository.clone();
            let result = cx
                .background_spawn(async move {
                    let keychain = MacKeychainStore;
                    let token = keychain
                        .read(&MacKeychainStore::github_key("default"))
                        .map_err(|_| HostingError::Network)?
                        .ok_or(HostingError::Authentication)?;
                    GitHubService::default().create_pull_request(
                        &token,
                        &worker_repository,
                        &fields[0],
                        &fields[1],
                        &fields[2],
                        &fields[3],
                    )
                })
                .await;
            let _ = this.update(cx, |app, cx| {
                match result {
                    Ok(request) => {
                        app.set_activity(format!("Created pull request #{}.", request.number));
                        app.load_pull_requests(repository, cx);
                    }
                    Err(error) => {
                        app.set_activity(format!("Create pull request failed: {error:?}"));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn prompt_comment_pull_request(&mut self, cx: &mut Context<Self>) {
        let Some(index) = self.selected_pull_request else {
            self.set_activity("Select a pull request first.");
            cx.notify();
            return;
        };
        let Some(repository) = self.pull_request_repository.clone() else {
            return;
        };
        let Some(request) = self.pull_requests.get(index) else {
            return;
        };
        let number = request.number;
        cx.spawn(async move |this, cx| {
            let body = cx
                .background_spawn(async {
                    Command::new("osascript")
                        .args(["-e", "text returned of (display dialog \"Comment on pull request\" default answer \"\")"])
                        .output()
                        .ok()
                        .filter(|output| output.status.success())
                        .map(|output| String::from_utf8_lossy(&output.stdout).trim_end().to_owned())
                        .filter(|body| !body.is_empty())
                })
                .await;
            let Some(body) = body else {
                return;
            };
            let worker_repository = repository.clone();
            let result = cx
                .background_spawn(async move {
                    let keychain = MacKeychainStore;
                    let token = keychain
                        .read(&MacKeychainStore::github_key("default"))
                        .map_err(|_| HostingError::Network)?
                        .ok_or(HostingError::Authentication)?;
                    GitHubService::default()
                        .comment_pull_request(&token, &worker_repository, number, &body)
                })
                .await;
            let _ = this.update(cx, |app, cx| {
                match result {
                    Ok(_) => {
                        app.set_activity(format!("Commented on pull request #{number}."));
                        app.load_pull_requests(repository, cx);
                    }
                    Err(error) => app.set_activity(format!("Comment failed: {error:?}")),
                }
                cx.notify();
            });
        })
        .detach();
    }

    #[allow(clippy::too_many_lines)]
    fn prompt_merge_pull_request(&mut self, cx: &mut Context<Self>) {
        let Some(index) = self.selected_pull_request else {
            self.set_activity("Select a pull request first.");
            cx.notify();
            return;
        };
        let Some(request) = self.pull_requests.get(index) else {
            return;
        };
        let number = request.number;
        self.begin_choice_prompt(ChoicePromptKind::MergePullRequest { number }, cx);
    }

    fn checkout_pull_request(&mut self, cx: &mut Context<Self>) {
        let Some(index) = self.selected_pull_request else {
            self.set_activity("Select a pull request first.");
            cx.notify();
            return;
        };
        let Some(request) = self.pull_requests.get(index) else {
            return;
        };
        let number = request.number;
        let branch = format!("pr/{number}");
        let Some(remote) = self.default_remote() else {
            self.set_activity("Add a Git remote before checking out a pull request.");
            cx.notify();
            return;
        };
        let ShellState::Repository(repository) = &self.state else {
            return;
        };
        let repository = repository.clone();
        let worker_repository = repository.clone();
        let worker_branch = branch.clone();
        self.mutation_in_flight = true;
        self.set_activity(format!("Checking out pull request #{number}…"));
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    let git = GitExecutable::discover().map_err(|error| error.to_string())?;
                    git.fetch_pull_request(&worker_repository, &remote, number)
                        .map_err(|error| format!("{error:?}"))?;
                    let start = format!("refs/remotes/{remote}/pr/{number}");
                    git.create_branch(&worker_repository, &worker_branch, Some(&start))
                        .map_err(|error| format!("{error:?}"))
                })
                .await;
            let _ = this.update(cx, |app, cx| {
                app.mutation_in_flight = false;
                match result {
                    Ok(()) => {
                        app.set_activity(format!(
                            "Checked out pull request #{number} as {branch}."
                        ));
                        app.load_working_copy(repository.clone(), cx);
                        GitronimoApp::load_refs(repository, cx);
                    }
                    Err(error) => app.set_activity(format!("Checkout failed: {error}")),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn prompt_submodule_update(path: Option<GitPath>, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let confirmed = cx
                .background_spawn(async {
                    Command::new("osascript")
                        .args(["-e", "button returned of (display dialog \"Update submodule to its configured commit?\" with title \"Gitronimo\" buttons {\"Cancel\", \"Update\"} default button \"Update\" with icon caution)"])
                        .output()
                        .ok()
                        .filter(|output| output.status.success())
                        .map(|output| String::from_utf8_lossy(&output.stdout).trim_end().to_owned())
                })
                .await;
            let _ = this.update(cx, |app, cx| {
                if confirmed.as_deref() != Some("Update") {
                    return;
                }
                app.run_worktree_mutation(
                    "Update submodule".to_owned(),
                    move |git, repository| git.submodule_update(repository, path.as_ref()),
                    cx,
                );
            });
        })
        .detach();
    }

    #[allow(dead_code)]
    fn prompt_open_submodule(path: GitPath, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let _ = this.update(cx, |app, _| {
                let ShellState::Repository(repository) = &app.state else {
                    return;
                };
                let absolute = repository
                    .worktree_root
                    .join(PathBuf::from(String::from_utf8_lossy(&path.0).into_owned()));
                let _ = Command::new("open").arg(&absolute).spawn();
            });
        })
        .detach();
    }

    fn show_rebase(&mut self, repository: WorktreeRepository, cx: &mut Context<Self>) {
        self.navigate_to(RepositoryView::Rebase, cx);
        self.load_rebase_plan(repository, cx);
    }

    fn load_rebase_plan(&mut self, repository: WorktreeRepository, cx: &mut Context<Self>) {
        let root = repository.worktree_root.clone();
        let load_token = self.rebase_plan_load_token;
        self.set_activity("Loading rebase plan…");
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    let git = GitExecutable::discover().map_err(|error| error.to_string())?;
                    git.rebase_plan(&repository)
                        .map_err(|error| format!("{error:?}"))
                })
                .await;
            let _ = this.update(cx, |app, cx| {
                if !matches!(
                    &app.state,
                    ShellState::Repository(current) if current.worktree_root == root
                ) || app.rebase_plan_load_token != load_token
                {
                    return;
                }
                match result {
                    Ok(plan) => {
                        app.rebase_plan = plan;
                        app.set_activity(format!(
                            "Loaded {} rebase step(s).",
                            app.rebase_plan.len()
                        ));
                    }
                    Err(error) => {
                        app.rebase_plan = Vec::new();
                        app.set_activity(format!("Rebase plan load failed: {error}"));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn reload_rebase_plan(&mut self, repository: &WorktreeRepository, cx: &mut Context<Self>) {
        self.rebase_plan_load_token = self.rebase_plan_load_token.wrapping_add(1);
        self.load_rebase_plan(repository.clone(), cx);
    }

    fn save_rebase_plan(cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let _ = this.update(cx, |app, cx| {
                let plan = app.rebase_plan.clone();
                if plan.is_empty() {
                    return;
                }
                app.run_worktree_mutation(
                    "Save rebase plan".to_owned(),
                    move |git, repository| git.save_rebase_plan(repository, &plan),
                    cx,
                );
            });
        })
        .detach();
    }

    fn prompt_start_rebase(&mut self, cx: &mut Context<Self>) {
        self.begin_text_prompt(TextPromptKind::RebaseOnto, "main", cx);
    }

    fn prompt_autosquash(&mut self, squash: bool, cx: &mut Context<Self>) {
        self.begin_text_prompt(TextPromptKind::AutosquashTarget { squash }, "HEAD", cx);
    }

    fn prompt_drop_commit(&mut self, cx: &mut Context<Self>) {
        self.begin_text_prompt(TextPromptKind::DropCommit, "HEAD~1", cx);
    }

    pub(crate) fn prompt_reword_last_commit(&mut self, cx: &mut Context<Self>) {
        self.begin_text_prompt(TextPromptKind::RewordSubject, "", cx);
    }

    fn show_conflicts(&mut self, repository: WorktreeRepository, cx: &mut Context<Self>) {
        self.navigate_to(RepositoryView::Conflicts, cx);
        self.conflict_path = None;
        self.conflict_content = None;
        self.load_working_copy(repository, cx);
    }

    fn view_conflict(repository: WorktreeRepository, path: GitPath, cx: &mut Context<Self>) {
        let worker_repository = repository.clone();
        let worker_path = path.clone();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    let git = GitExecutable::discover().map_err(|error| error.to_string())?;
                    git.read_working_file(&worker_repository, &worker_path)
                        .map_err(|error| format!("{error:?}"))
                })
                .await;
            let _ = this.update(cx, |app, cx| {
                if !matches!(&app.state, ShellState::Repository(current) if current.worktree_root == repository.worktree_root)
                {
                    return;
                }
                app.conflict_path = Some(path.clone());
                match result {
                    Ok(content) => app.conflict_content = Some(content),
                    Err(error) => {
                        app.conflict_content = None;
                        app.set_activity(format!("Conflict read failed: {error}"));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn resolve_conflict(path: GitPath, side: ConflictSide, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let _ = this.update(cx, |app, cx| {
                let side_label = match side {
                    ConflictSide::Ours => "ours",
                    ConflictSide::Theirs => "theirs",
                };
                app.run_worktree_mutation(
                    format!("Resolve with {side_label}"),
                    move |git, repository| git.resolve_conflict(repository, &path, side),
                    cx,
                );
            });
        })
        .detach();
    }

    fn prompt_set_merge_tool(&mut self, cx: &mut Context<Self>) {
        self.begin_choice_prompt(ChoicePromptKind::SetMergeTool, cx);
    }

    fn prompt_run_merge_tool(&mut self, cx: &mut Context<Self>) {
        self.begin_text_prompt(TextPromptKind::MergeToolPath, "", cx);
    }

    fn prompt_check_commit_signature(cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let default_oid = this
                .read_with(cx, |app, _| {
                    if app.repository_view == RepositoryView::History {
                        app.selected_history
                            .and_then(|index| app.history.get(index))
                            .map(|commit| commit.oid.clone())
                    } else {
                        None
                    }
                })
                .ok()
                .flatten();
            let default_oid = default_oid.unwrap_or_else(|| "HEAD".to_owned());
            let oid = cx
                .background_spawn(async move {
                    Command::new("osascript")
                        .args([
                            "-e",
                            &format!(
                                "text returned of (display dialog \"Commit to verify\" default answer \"{default_oid}\")"
                            ),
                        ])
                        .output()
                        .ok()
                        .filter(|output| output.status.success())
                        .map(|output| String::from_utf8_lossy(&output.stdout).trim_end().to_owned())
                        .filter(|oid| !oid.is_empty())
                })
                .await;
            let _ = this.update(cx, |app, cx| {
                let Some(oid) = oid else {
                    return;
                };
                let ShellState::Repository(repository) = &app.state else {
                    app.set_activity("Open a repository before verifying a commit.");
                    return;
                };
                let repository = repository.clone();
                let worker_repository = repository.clone();
                let worker_oid = oid.clone();
                app.set_activity(format!("Checking signature of {oid}…"));
                cx.spawn(async move |this, cx| {
                    let result = cx
                        .background_spawn(async move {
                            let git =
                                GitExecutable::discover().map_err(|error| error.to_string())?;
                            git.commit_signature(&worker_repository, &worker_oid)
                                .map_err(|error| format!("{error:?}"))
                        })
                        .await;
                    let _ = this.update(cx, |app, cx| {
                        match result {
                            Ok(signature) => {
                                let label = signature.status.label();
                                app.set_activity(if signature.signer.is_empty() {
                                    format!("Commit {oid} signature: {label}.")
                                } else {
                                    format!(
                                        "Commit {oid} signature: {label} (signed by {}).",
                                        signature.signer
                                    )
                                });
                            }
                            Err(error) => {
                                app.set_activity(git_failure_message(&format!("Verify commit {oid}"), &error));
                            }
                        }
                        cx.notify();
                    });
                })
                .detach();
            });
        })
        .detach();
    }

    fn toggle_appearance(
        &mut self,
        _: &ToggleAppearance,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.theme_mode = match self.theme_mode {
            ThemeMode::System => ThemeMode::Light,
            ThemeMode::Light => ThemeMode::Dark,
            ThemeMode::Dark => ThemeMode::System,
        };
        self.apply_theme_mode(Some(window), cx);
    }

    pub(crate) fn apply_theme_mode(&mut self, window: Option<&Window>, cx: &mut Context<Self>) {
        self.appearance = match self.theme_mode {
            ThemeMode::System => window.map_or(self.appearance, |window| {
                appearance_from_window(window.appearance())
            }),
            ThemeMode::Light => Appearance::Light,
            ThemeMode::Dark => Appearance::Dark,
        };
        cx.notify();
    }

    fn widen_sidebar(&mut self, _: &WidenSidebar, _: &mut Window, cx: &mut Context<Self>) {
        self.sidebar_width = resize_width(self.sidebar_width);
        let _ = self.store.save_sidebar_width(self.sidebar_width);
        cx.notify();
    }

    fn dropped_paths(
        &mut self,
        paths: &ExternalPaths,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(path) = paths.paths().first() else {
            return;
        };
        self.open_path(path.clone(), window, cx);
    }

    fn open_recent(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        self.open_path(path, window, cx);
    }

    fn select_recent(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < self.recents.len() {
            self.selected_recent = Some(index);
            self.reload_workflow_from_selection();
            self.refresh_welcome_snapshot(cx);
            cx.notify();
        }
    }

    fn refresh_welcome_snapshot(&mut self, cx: &mut Context<Self>) {
        let Some(index) = self.selected_recent else {
            self.welcome_snapshot = None;
            self.welcome_snapshot_path = None;
            return;
        };
        let Some(path) = self.recents.get(index).cloned() else {
            self.welcome_snapshot = None;
            self.welcome_snapshot_path = None;
            return;
        };
        if self.welcome_snapshot_path.as_ref() == Some(&path) {
            return;
        }
        self.welcome_snapshot = None;
        self.welcome_snapshot_path = Some(path.clone());
        self.welcome_snapshot_token = self.welcome_snapshot_token.wrapping_add(1);
        let token = self.welcome_snapshot_token;
        let list_path = path.clone();
        cx.spawn(async move |this, cx| {
            let snapshot = cx
                .background_spawn(async move { load_welcome_snapshot(&path) })
                .await;
            let _ = this.update(cx, |app, cx| {
                if app.welcome_snapshot_token != token {
                    return;
                }
                app.welcome_snapshot = Some(snapshot.clone());
                app.welcome_list_snapshots.insert(list_path, snapshot);
                cx.notify();
            });
        })
        .detach();
    }

    fn refresh_welcome_list_snapshots(&mut self, cx: &mut Context<Self>) {
        let paths = self.recents.clone();
        self.welcome_list_snapshot_token = self.welcome_list_snapshot_token.wrapping_add(1);
        let token = self.welcome_list_snapshot_token;
        cx.spawn(async move |this, cx| {
            let snapshots = cx
                .background_spawn(async move {
                    paths
                        .into_iter()
                        .map(|path| {
                            let snapshot = load_welcome_snapshot(&path);
                            (path, snapshot)
                        })
                        .collect::<std::collections::HashMap<_, _>>()
                })
                .await;
            let _ = this.update(cx, |app, cx| {
                if app.welcome_list_snapshot_token != token {
                    return;
                }
                app.welcome_list_snapshots = snapshots;
                cx.notify();
            });
        })
        .detach();
    }

    fn open_selected_recent(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(index) = self.selected_recent else {
            return;
        };
        let Some(path) = self.recents.get(index).cloned() else {
            return;
        };
        self.open_recent(path, window, cx);
    }

    fn confirm_remove_selected_recent(&mut self, cx: &mut Context<Self>) {
        let Some(index) = self.selected_recent else {
            return;
        };
        let Some(path) = self.recents.get(index).cloned() else {
            return;
        };
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("this repository")
            .to_owned();
        let dialog_name = name.clone();
        self.set_activity(format!("Remove {name} from recents?"));
        let store = self.store.clone();
        cx.spawn(async move |this, cx| {
            let confirmed = cx
                .background_spawn(async move {
                    Command::new("osascript")
                        .args(["-e", &format!(
                            "button returned of (display dialog \"Remove {dialog_name} from Gitronimo's repository list?\" with title \"Gitronimo\" buttons {{\"Cancel\", \"Remove\"}} default button \"Cancel\" cancel button \"Cancel\" with icon caution)"
                        )])
                        .output()
                        .ok()
                        .is_some_and(|output| {
                            output.status.success()
                                && String::from_utf8_lossy(&output.stdout).trim() == "Remove"
                        })
                })
                .await;
            let _ = this.update(cx, |app, cx| {
                if !confirmed {
                    app.set_activity("Kept the repository in recents.");
                    cx.notify();
                    return;
                }
                app.recents = store.remove(&path).unwrap_or_else(|_| {
                    app.recents.retain(|recent| recent != &path);
                    app.recents.clone()
                });
                app.selected_recent = if app.recents.is_empty() {
                    None
                } else {
                    Some(index.min(app.recents.len().saturating_sub(1)))
                };
                app.welcome_snapshot = None;
                app.welcome_snapshot_path = None;
                app.set_activity(format!("Removed {name} from recents."));
                if app.selected_recent.is_some() {
                    app.refresh_welcome_snapshot(cx);
                }
                app.refresh_welcome_list_snapshots(cx);
                cx.notify();
            });
        })
        .detach();
    }

    fn open_path(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        if self.has_commit_draft() {
            self.confirm_draft_discard_before_open(path, window, cx);
            return;
        }
        self.begin_open_path(path, window, cx);
    }

    fn confirm_draft_discard_before_open(
        &mut self,
        path: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.set_activity("Discard the unsaved commit draft before opening another repository?");
        cx.spawn_in(window, async move |this, cx| {
            let confirmed = cx
                .background_spawn(async {
                    Command::new("osascript")
                        .args(["-e", "display dialog \"Discard the unsaved commit draft before opening another repository?\" buttons {\"Cancel\", \"Discard draft and open\"} default button \"Cancel\" cancel button \"Cancel\""])
                        .output()
                        .ok()
                        .is_some_and(|output| output.status.success())
                })
                .await;
            let _ = this.update_in(cx, |app, window, cx| {
                if confirmed {
                    app.commit_subject.clear();
                    app.commit_body.clear();
                    app.refresh_commit_inputs(cx);
                    app.begin_open_path(path, window, cx);
                } else {
                    app.set_activity("Kept the unsaved commit draft.");
                    cx.notify();
                }
            });
        })
        .detach();
    }

    fn begin_open_path(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        if matches!(self.state, ShellState::Welcome) {
            self.came_from_welcome = true;
        }
        self.state = ShellState::Loading(path.clone());
        self.stop_watcher();
        self.set_activity("Opening repository…");
        let store = RecentRepositoryStore::new(preferences_path());
        cx.spawn_in(window, async move |this, cx| {
            let outcome = cx
                .background_spawn(async move { discover_and_record(&path, &store) })
                .await;
            let _ = this.update_in(cx, |app, _, cx| app.apply_open_outcome(outcome, cx));
        })
        .detach();
        cx.notify();
    }

    fn load_working_copy(&mut self, repository: WorktreeRepository, cx: &mut Context<Self>) {
        if !repository_is_available(&repository) {
            self.stop_watcher();
            self.state = ShellState::Error(repository_unavailable_message(&repository));
            self.set_activity(
                "Repository is no longer available. Choose it again when it returns.",
            );
            cx.notify();
            return;
        }
        let root = repository.worktree_root.clone();
        self.set_activity("Refreshing working copy…");
        cx.spawn(async move |this, cx| {
            let refresh = cx
                .background_spawn(async move {
                    let git = GitExecutable::discover().map_err(|error| error.to_string())?;
                    let status = git
                        .worktree_status(&repository, false)
                        .map_err(|error| format!("{error:?}"))?;
                    let numstat = git.diff_numstat(&repository).ok();
                    let last_commit = git
                        .history_page(
                            &repository,
                            &HistoryRequest {
                                reference: HistoryReference::Current,
                                before: None,
                                limit: 1,
                            },
                        )
                        .ok()
                        .and_then(|page| page.commits.first().cloned())
                        .map(|commit| String::from_utf8_lossy(&commit.subject).into_owned());
                    Ok::<_, String>((status, numstat, last_commit))
                })
                .await;
            let _ = this.update(cx, |app, cx| {
                let ShellState::Repository(current) = &app.state else {
                    return;
                };
                if current.worktree_root != root {
                    return;
                }
                match refresh {
                    Ok((status, numstat, last_commit)) => {
                        app.set_activity(format!(
                            "Working copy refreshed: {} change(s).",
                            status.entries.len()
                        ));
                        app.working_copy = Some(status);
                        if let Some(stats) = numstat {
                            app.file_diff_stats = stats
                                .into_iter()
                                .filter_map(|(path, (added, deleted))| {
                                    let added = usize::try_from(added).ok()?;
                                    let deleted = usize::try_from(deleted).ok()?;
                                    Some((path, (added, deleted)))
                                })
                                .collect();
                        }
                        if let Some(summary) = last_commit {
                            app.last_commit_summary = Some(summary);
                        }
                    }
                    Err(error) => app.set_activity(format!("Working copy refresh failed: {error}")),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn load_refs(repository: WorktreeRepository, cx: &mut Context<Self>) {
        let root = repository.worktree_root.clone();
        cx.spawn(async move |this, cx| {
            let refs = cx
                .background_spawn(async move {
                    GitExecutable::discover()
                        .map_err(|error| error.to_string())?
                        .ref_snapshot(&repository)
                        .map_err(|error| format!("{error:?}"))
                })
                .await;
            let _ = this.update(cx, |app, cx| {
                if matches!(&app.state, ShellState::Repository(current) if current.worktree_root == root) {
                    match refs {
                        Ok(refs) => {
                            if app.pending_branch_delete.as_ref().is_some_and(|branch| {
                                !refs
                                    .local_branches
                                    .iter()
                                    .any(|entry| entry.name.0 == branch.as_bytes())
                            }) {
                                app.pending_branch_delete = None;
                            }
                            app.refs = refs;
                        }
                        Err(error) => app.set_activity(format!("Ref load failed: {error}")),
                    }
                    cx.notify();
                }
            });
        })
        .detach();
    }

    #[allow(dead_code)]
    fn prompt_branch_name(create: bool, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let name = cx
                .background_spawn(async move {
                    let title = if create {
                        "New branch from HEAD"
                    } else {
                        "Checkout branch"
                    };
                    Command::new("osascript")
                        .args([
                            "-e",
                            &format!(
                                "text returned of (display dialog \"{title}\" default answer \"\")"
                            ),
                        ])
                        .output()
                        .ok()
                        .filter(|output| output.status.success())
                        .map(|output| {
                            String::from_utf8_lossy(&output.stdout)
                                .trim_end()
                                .to_owned()
                        })
                        .filter(|name| !name.is_empty())
                })
                .await;
            let _ = this.update(cx, |app, cx| {
                let Some(name) = name else {
                    return;
                };
                if create {
                    app.create_branch(name, cx);
                } else {
                    app.checkout_branch(name, cx);
                }
            });
        })
        .detach();
    }

    fn checkout_branch(&mut self, branch: String, cx: &mut Context<Self>) {
        self.run_branch_command(
            format!("Checking out {branch}"),
            move |git, repository| git.checkout_branch(repository, &branch),
            cx,
        );
    }

    #[allow(dead_code)]
    fn create_branch(&mut self, branch: String, cx: &mut Context<Self>) {
        self.create_branch_from(branch, None, cx);
    }

    #[allow(dead_code)]
    fn prompt_rename_current_branch(cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let name = cx
                .background_spawn(async {
                    Command::new("osascript")
                        .args(["-e", "text returned of (display dialog \"Rename current branch\" default answer \"\")"])
                        .output()
                        .ok()
                        .filter(|output| output.status.success())
                        .map(|output| String::from_utf8_lossy(&output.stdout).trim_end().to_owned())
                        .filter(|name| !name.is_empty())
                })
                .await;
            let _ = this.update(cx, |app, cx| {
                let Some(name) = name else { return; };
                let Some(current) = app.working_copy.as_ref().and_then(|status| match &status.branch.head {
                    HeadStatus::Branch(branch) => String::from_utf8(branch.0.clone()).ok(),
                    _ => None,
                }) else {
                    app.set_activity("Checkout a local branch before renaming it.");
                    cx.notify();
                    return;
                };
                app.run_branch_command(format!("Renaming {current} to {name}"), move |git, repository| {
                    git.rename_branch(repository, &current, &name)
                }, cx);
            });
        }).detach();
    }

    #[allow(dead_code)]
    fn prompt_delete_local_branch(cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let branch = cx
                .background_spawn(async {
                    Command::new("osascript")
                        .args(["-e", "text returned of (display dialog \"Delete local branch\" default answer \"\")"])
                        .output()
                        .ok()
                        .filter(|output| output.status.success())
                        .map(|output| String::from_utf8_lossy(&output.stdout).trim_end().to_owned())
                        .filter(|branch| !branch.is_empty())
                })
                .await;
            let _ = this.update(cx, |app, cx| {
                let Some(branch) = branch else { return; };
                if !app
                    .refs
                    .local_branches
                    .iter()
                    .any(|entry| entry.name.0 == branch.as_bytes())
                {
                    app.set_activity(format!("Unknown local branch: {branch}"));
                    cx.notify();
                    return;
                }
                app.pending_branch_delete = Some(branch.clone());
                app.set_activity(format!("Review deletion choices for local branch {branch}."));
                cx.notify();
            });
        }).detach();
    }

    pub(crate) fn confirm_branch_delete(&mut self, force: bool, cx: &mut Context<Self>) {
        let Some(branch) = self.pending_branch_delete.take() else {
            return;
        };
        self.confirm_dialog = None;
        if self.mutation_in_flight {
            self.pending_branch_delete = Some(branch);
            return;
        }
        let ShellState::Repository(repository) = &self.state else {
            self.pending_branch_delete = Some(branch);
            return;
        };
        let repository = repository.clone();
        let worker_repository = repository.clone();
        let branch_name = branch.clone();
        let label = if force {
            format!("Force deleting unmerged branch {branch}")
        } else {
            format!("Deleting branch {branch}")
        };
        self.mutation_in_flight = true;
        self.set_activity(format!("{label}…"));
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    let git = GitExecutable::discover().map_err(|error| error.to_string())?;
                    git.delete_branch(&worker_repository, &branch_name, force)
                        .map_err(|error| format!("{error:?}"))
                })
                .await;
            let _ = this.update(cx, |app, cx| {
                app.mutation_in_flight = false;
                match result {
                    Ok(()) => {
                        app.set_activity(format!("{label} complete."));
                        app.load_working_copy(repository.clone(), cx);
                        Self::load_refs(repository, cx);
                    }
                    Err(error) if !force && branch_not_fully_merged_error(&error) => {
                        app.confirm_dialog = Some(AppConfirmDialog::ForceDeleteBranch {
                            branch: branch.clone(),
                        });
                        app.set_activity(format!(
                            "Could not delete \"{branch}\": it contains unmerged changes."
                        ));
                    }
                    Err(error) => app.set_activity(git_failure_message(&label, &error)),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn create_branch_from(
        &mut self,
        branch: String,
        start: Option<String>,
        cx: &mut Context<Self>,
    ) {
        self.run_branch_command(
            format!("Creating {branch}"),
            move |git, repository| git.create_branch(repository, &branch, start.as_deref()),
            cx,
        );
    }

    fn default_remote(&self) -> Option<String> {
        self.refs
            .remotes
            .first()
            .and_then(|remote| String::from_utf8(remote.name.0.clone()).ok())
    }

    #[allow(dead_code)]
    fn has_attached_branch(&self) -> bool {
        self.working_copy
            .as_ref()
            .is_some_and(|status| matches!(status.branch.head, HeadStatus::Branch(_)))
    }

    fn fetch_default_remote(&mut self, cx: &mut Context<Self>) {
        let Some(remote) = self.default_remote() else {
            self.set_activity("No configured remote to fetch.");
            cx.notify();
            return;
        };
        self.run_network_command(
            format!("Fetching {remote}"),
            vec!["fetch".into(), "--progress".into(), remote.into()],
            cx,
        );
    }

    #[allow(dead_code)]
    fn prompt_fetch_remote(cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let remote = cx.background_spawn(async {
                Command::new("osascript")
                    .args(["-e", "text returned of (display dialog \"Fetch configured remote\" default answer \"\")"])
                    .output().ok().filter(|output| output.status.success())
                    .map(|output| String::from_utf8_lossy(&output.stdout).trim_end().to_owned())
                    .filter(|name| !name.is_empty())
            }).await;
            let _ = this.update(cx, |app, cx| {
                let Some(remote) = remote else { return; };
                if !app.refs.remotes.iter().any(|entry| entry.name.0 == remote.as_bytes()) {
                    app.set_activity(format!("Unknown configured remote: {remote}"));
                    cx.notify();
                    return;
                }
                app.run_network_command(
                    format!("Fetching {remote}"),
                    vec!["fetch".into(), "--progress".into(), remote.into()],
                    cx,
                );
            });
        }).detach();
    }

    pub(crate) fn pull_current(&mut self, cx: &mut Context<Self>) {
        self.open_pull_dialog(None, cx);
    }

    pub(crate) fn pull_branch(&mut self, branch: String, cx: &mut Context<Self>) {
        // Prefer matching remote-tracking name when a local branch was passed.
        let preferred = self
            .default_remote()
            .map(|remote| format!("{remote}/{branch}"))
            .filter(|candidate| {
                self.refs
                    .remote_branches
                    .iter()
                    .any(|entry| String::from_utf8_lossy(&entry.name.0) == candidate.as_str())
            })
            .or_else(|| {
                self.refs.remote_branches.iter().find_map(|entry| {
                    let name = String::from_utf8(entry.name.0.clone()).ok()?;
                    (name == branch).then_some(name)
                })
            })
            .or(Some(branch));
        self.open_pull_dialog(preferred, cx);
    }

    pub(crate) fn sync_current(&mut self, cx: &mut Context<Self>) {
        self.fetch_default_remote(cx);
        self.run_network_command(
            "Pulling current branch".into(),
            vec!["pull".into(), "--progress".into()],
            cx,
        );
        self.run_network_command(
            "Pushing current branch".into(),
            vec!["push".into(), "--progress".into()],
            cx,
        );
    }

    pub(crate) fn open_pull_dialog(
        &mut self,
        preferred_remote_branch: Option<String>,
        cx: &mut Context<Self>,
    ) {
        if !matches!(self.state, ShellState::Repository(_)) {
            return;
        }
        let remote_branches = self.remote_branch_choices();
        let remote_branch = preferred_remote_branch
            .filter(|name| remote_branches.iter().any(|candidate| candidate == name))
            .or_else(|| self.default_pull_remote_branch(&remote_branches))
            .unwrap_or_default();
        self.close_ref_context_menu(cx);
        self.pull_dialog = Some(PullDialogState {
            use_rebase: false,
            remote_branch,
            remote_branches,
            branch_menu_open: false,
        });
        cx.notify();
    }

    pub(crate) fn close_pull_dialog(&mut self, cx: &mut Context<Self>) {
        if self.pull_dialog.take().is_some() {
            cx.notify();
        }
    }

    pub(crate) fn toggle_pull_dialog_rebase(&mut self, cx: &mut Context<Self>) {
        if let Some(dialog) = self.pull_dialog.as_mut() {
            dialog.use_rebase = !dialog.use_rebase;
            cx.notify();
        }
    }

    pub(crate) fn toggle_pull_dialog_branch_menu(&mut self, cx: &mut Context<Self>) {
        if let Some(dialog) = self.pull_dialog.as_mut() {
            dialog.branch_menu_open = !dialog.branch_menu_open;
            cx.notify();
        }
    }

    pub(crate) fn select_pull_dialog_remote_branch(
        &mut self,
        remote_branch: String,
        cx: &mut Context<Self>,
    ) {
        if let Some(dialog) = self.pull_dialog.as_mut() {
            dialog.remote_branch = remote_branch;
            dialog.branch_menu_open = false;
            cx.notify();
        }
    }

    pub(crate) fn confirm_pull_dialog(&mut self, cx: &mut Context<Self>) {
        let Some(dialog) = self.pull_dialog.take() else {
            return;
        };
        let remotes = self.configured_remote_names();
        let args = pull_command_args(&dialog.remote_branch, dialog.use_rebase, &remotes);
        let label = if dialog.remote_branch.is_empty() {
            if dialog.use_rebase {
                "Pulling current branch (rebase)".into()
            } else {
                "Pulling current branch".into()
            }
        } else if dialog.use_rebase {
            format!("Pulling {} (rebase)", dialog.remote_branch)
        } else {
            format!("Pulling {}", dialog.remote_branch)
        };
        self.run_network_command(label, args, cx);
        cx.notify();
    }

    pub(crate) fn remote_branch_choices(&self) -> Vec<String> {
        let mut names: Vec<String> = self
            .refs
            .remote_branches
            .iter()
            .filter_map(|branch| String::from_utf8(branch.name.0.clone()).ok())
            .collect();
        names.sort();
        names.dedup();
        names
    }

    pub(crate) fn configured_remote_names(&self) -> Vec<String> {
        self.refs
            .remotes
            .iter()
            .filter_map(|remote| String::from_utf8(remote.name.0.clone()).ok())
            .collect()
    }

    fn default_pull_remote_branch(&self, remote_branches: &[String]) -> Option<String> {
        let head = self
            .working_copy
            .as_ref()
            .and_then(|status| match &status.branch.head {
                HeadStatus::Branch(branch) => String::from_utf8(branch.0.clone()).ok(),
                _ => None,
            });
        if let Some(head) = head.as_ref() {
            if let Some(local) = self
                .refs
                .local_branches
                .iter()
                .find(|branch| String::from_utf8_lossy(&branch.name.0) == head.as_str())
                && let Some(upstream) = local.upstream.clone()
                && remote_branches.iter().any(|name| name == &upstream)
            {
                return Some(upstream);
            }
            if let Some(remote) = self.default_remote() {
                let candidate = format!("{remote}/{head}");
                if remote_branches.iter().any(|name| name == &candidate) {
                    return Some(candidate);
                }
            }
        }
        remote_branches.first().cloned()
    }

    pub(crate) fn push_current(&mut self, cx: &mut Context<Self>) {
        self.open_push_dialog(None, cx);
    }

    pub(crate) fn push_branch(&mut self, branch: &str, cx: &mut Context<Self>) {
        let preferred = self
            .default_remote()
            .map(|remote| format!("{remote}/{branch}"));
        self.open_push_dialog(preferred, cx);
    }

    pub(crate) fn open_push_dialog(
        &mut self,
        preferred_destination: Option<String>,
        cx: &mut Context<Self>,
    ) {
        if !matches!(self.state, ShellState::Repository(_)) {
            return;
        }
        let head_branch = self.head_branch_name();
        let mut destinations = self.remote_branch_choices();
        if let Some(candidate) = self.proposed_push_destination(head_branch.as_deref())
            && !destinations.iter().any(|name| name == &candidate)
        {
            destinations.insert(0, candidate);
        }
        let destination = preferred_destination
            .filter(|name| destinations.iter().any(|candidate| candidate == name))
            .or_else(|| self.default_push_destination(&destinations, head_branch.as_deref()))
            .unwrap_or_default();
        self.close_ref_context_menu(cx);
        self.push_dialog = Some(PushDialogState {
            head_branch: head_branch.unwrap_or_else(|| "HEAD".into()),
            destination,
            destinations,
            destination_menu_open: false,
            enabled_options: Vec::new(),
            submodule_mode: SubmodulePushMode::Check,
            submodule_menu_open: false,
        });
        cx.notify();
    }

    pub(crate) fn close_push_dialog(&mut self, cx: &mut Context<Self>) {
        if self.push_dialog.take().is_some() {
            cx.notify();
        }
    }

    pub(crate) fn toggle_push_dialog_destination_menu(&mut self, cx: &mut Context<Self>) {
        if let Some(dialog) = self.push_dialog.as_mut() {
            dialog.destination_menu_open = !dialog.destination_menu_open;
            dialog.submodule_menu_open = false;
            cx.notify();
        }
    }

    pub(crate) fn select_push_dialog_destination(
        &mut self,
        destination: String,
        cx: &mut Context<Self>,
    ) {
        if let Some(dialog) = self.push_dialog.as_mut() {
            dialog.destination = destination;
            dialog.destination_menu_open = false;
            cx.notify();
        }
    }

    pub(crate) fn toggle_push_dialog_option(&mut self, option: PushOption, cx: &mut Context<Self>) {
        if let Some(dialog) = self.push_dialog.as_mut() {
            dialog.toggle(option);
            if option == PushOption::RecurseSubmodules
                && !dialog.is_enabled(PushOption::RecurseSubmodules)
            {
                dialog.submodule_menu_open = false;
            }
            cx.notify();
        }
    }

    pub(crate) fn toggle_push_dialog_submodule_menu(&mut self, cx: &mut Context<Self>) {
        if let Some(dialog) = self.push_dialog.as_mut()
            && dialog.is_enabled(PushOption::RecurseSubmodules)
        {
            dialog.submodule_menu_open = !dialog.submodule_menu_open;
            dialog.destination_menu_open = false;
            cx.notify();
        }
    }

    pub(crate) fn select_push_dialog_submodule_mode(
        &mut self,
        mode: SubmodulePushMode,
        cx: &mut Context<Self>,
    ) {
        if let Some(dialog) = self.push_dialog.as_mut() {
            dialog.submodule_mode = mode;
            dialog.submodule_menu_open = false;
            cx.notify();
        }
    }

    pub(crate) fn confirm_push_dialog(&mut self, cx: &mut Context<Self>) {
        let Some(dialog) = self.push_dialog.take() else {
            return;
        };
        let remotes = self.configured_remote_names();
        let tracked = self.remote_branch_choices();
        let options = PushCommandOptions {
            head_branch: dialog.head_branch.clone(),
            enabled: dialog.enabled_options.clone(),
            submodule_mode: dialog.submodule_mode,
            set_upstream: !dialog.destination.is_empty()
                && !tracked.iter().any(|name| name == &dialog.destination),
        };
        let args = push_command_args(&dialog.destination, &options, &remotes);
        let label = if dialog.destination.is_empty() {
            format!("Pushing {}", dialog.head_branch)
        } else {
            format!("Pushing {} to {}", dialog.head_branch, dialog.destination)
        };
        self.force_push_state = ForcePushState::Idle;
        self.run_network_command(label, args, cx);
        cx.notify();
    }

    pub(crate) fn head_branch_name(&self) -> Option<String> {
        self.working_copy
            .as_ref()
            .and_then(|status| match &status.branch.head {
                HeadStatus::Branch(branch) => String::from_utf8(branch.0.clone()).ok(),
                HeadStatus::Detached | HeadStatus::Unborn | HeadStatus::Unknown => None,
            })
    }

    fn proposed_push_destination(&self, head_branch: Option<&str>) -> Option<String> {
        let head = head_branch?;
        let remote = self.default_remote()?;
        Some(format!("{remote}/{head}"))
    }

    fn default_push_destination(
        &self,
        destinations: &[String],
        head_branch: Option<&str>,
    ) -> Option<String> {
        if let Some(head) = head_branch {
            if let Some(local) = self
                .refs
                .local_branches
                .iter()
                .find(|branch| String::from_utf8_lossy(&branch.name.0) == head)
                && let Some(upstream) = local.upstream.clone()
                && destinations.iter().any(|name| name == &upstream)
            {
                return Some(upstream);
            }
            if let Some(candidate) = self.proposed_push_destination(Some(head))
                && destinations.iter().any(|name| name == &candidate)
            {
                return Some(candidate);
            }
        }
        destinations.first().cloned()
    }

    #[allow(dead_code)]
    fn publish_current(&mut self, cx: &mut Context<Self>) {
        let Some(remote) = self.default_remote() else {
            self.set_activity("No configured remote to publish to.");
            cx.notify();
            return;
        };
        let Some(branch) =
            self.working_copy
                .as_ref()
                .and_then(|status| match &status.branch.head {
                    HeadStatus::Branch(branch) => String::from_utf8(branch.0.clone()).ok(),
                    _ => None,
                })
        else {
            self.set_activity("Checkout a local branch before publishing.");
            cx.notify();
            return;
        };
        self.run_network_command(
            format!("Publishing {branch} to {remote}"),
            vec![
                "push".into(),
                "--progress".into(),
                "--set-upstream".into(),
                remote.into(),
                branch.into(),
            ],
            cx,
        );
    }

    pub(crate) fn request_force_with_lease(&mut self, cx: &mut Context<Self>) {
        self.force_push_state = ForcePushState::AwaitingConfirmation;
        self.set_activity("Force-with-lease can rewrite the remote branch. Review and confirm.");
        cx.notify();
    }

    fn confirm_force_with_lease(&mut self, cx: &mut Context<Self>) {
        self.force_push_state = ForcePushState::Idle;
        self.run_network_command(
            "Force pushing current branch with lease".into(),
            vec![
                "push".into(),
                "--progress".into(),
                "--force-with-lease".into(),
            ],
            cx,
        );
    }

    #[allow(clippy::too_many_lines)]
    fn run_network_command(&mut self, label: String, args: Vec<OsString>, cx: &mut Context<Self>) {
        if self.mutation_in_flight {
            return;
        }
        let ShellState::Repository(repository) = &self.state else {
            return;
        };
        let repository = repository.clone();
        let operation = Arc::new(Mutex::new(NetworkOperation {
            label: label.clone(),
            child: None,
            cancelled: false,
        }));
        let worker_operation = operation.clone();
        let worker_repository = repository.clone();
        self.mutation_in_flight = true;
        self.network_operation = Some(operation.clone());
        self.network_progress = 0.0;
        self.set_activity(format!("{label} in progress. You can cancel it."));
        cx.spawn(async move |this, cx| {
            let (progress_tx, progress_rx) = mpsc::channel::<f32>();
            let progress_this = this.clone();
            cx.spawn(async move |cx| {
                while let Ok(pct) = progress_rx.recv() {
                    let _ = progress_this.update(cx, |app, cx| {
                        app.network_progress = pct;
                        cx.notify();
                    });
                }
            })
            .detach();
            let result = cx
                .background_spawn(async move {
                    let git = GitExecutable::discover().map_err(|error| error.to_string())?;
                    let mut child = git
                        .start(&worker_repository.worktree_root, args)
                        .map_err(|error| error.to_string())?;
                    let stderr = child
                        .take_stderr()
                        .ok_or_else(|| "Git did not expose operation progress.".to_owned())?;
                    {
                        let mut operation = worker_operation
                            .lock()
                            .map_err(|_| "Network operation state was unavailable.".to_owned())?;
                        if operation.cancelled {
                            child.cancel().map_err(|error| error.to_string())?;
                        }
                        operation.child = Some(child);
                    }
                    let stderr_reader = thread::spawn(move || {
                        let reader = BufReader::new(stderr);
                        let mut output = String::new();
                        for line in reader.lines().map_while(Result::ok) {
                            output.push_str(&line);
                            output.push('\n');
                            if let Some(pct) = parse_git_progress_line(&line) {
                                progress_tx.send(pct).ok();
                            }
                        }
                        output
                    });
                    let mut operation = worker_operation
                        .lock()
                        .map_err(|_| "Network operation state was unavailable.".to_owned())?;
                    let cancelled = operation.cancelled;
                    let status = operation
                        .child
                        .as_mut()
                        .ok_or_else(|| "Git operation ended unexpectedly.".to_owned())?
                        .wait()
                        .map_err(|error| error.to_string())?;
                    let progress = stderr_reader
                        .join()
                        .map_err(|_| "Git progress reader stopped unexpectedly.".to_owned())?;
                    if cancelled {
                        Err("cancelled".to_owned())
                    } else if status.success() {
                        Ok(())
                    } else {
                        Err(progress)
                    }
                })
                .await;
            let _ = this.update(cx, |app, cx| {
                if !app
                    .network_operation
                    .as_ref()
                    .is_some_and(|current| Arc::ptr_eq(current, &operation))
                {
                    return;
                }
                app.network_operation = None;
                app.mutation_in_flight = false;
                app.network_progress = 0.0;
                match result {
                    Ok(()) => {
                        app.last_network_result = Some(format!("{label} complete."));
                        app.set_activity(app.last_network_result.clone().unwrap_or_default());
                        app.load_working_copy(repository.clone(), cx);
                        Self::load_refs(repository, cx);
                    }
                    Err(error) if error == "cancelled" => {
                        app.last_network_result = Some(format!("{label} cancelled."));
                        app.set_activity(app.last_network_result.clone().unwrap_or_default());
                    }
                    Err(error) => {
                        let message = network_failure_message(&label, &error);
                        app.last_network_result = Some(message.clone());
                        app.set_activity(message);
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn cancel_network_operation(&mut self, cx: &mut Context<Self>) {
        let Some(operation) = &self.network_operation else {
            return;
        };
        let cancelled = operation.lock().is_ok_and(|mut operation| {
            operation.cancelled = true;
            operation
                .child
                .as_mut()
                .is_none_or(|child| child.cancel().is_ok())
        });
        self.set_activity(if cancelled {
            "Cancelling network operation…"
        } else {
            "Unable to cancel the network operation."
        });
        cx.notify();
    }

    fn run_branch_command(
        &mut self,
        label: String,
        command: impl FnOnce(&GitExecutable, &WorktreeRepository) -> Result<(), GitStatusError>
        + Send
        + 'static,
        cx: &mut Context<Self>,
    ) {
        if self.mutation_in_flight {
            return;
        }
        let ShellState::Repository(repository) = &self.state else {
            return;
        };
        let repository = repository.clone();
        let worker_repository = repository.clone();
        self.mutation_in_flight = true;
        self.set_activity(format!("{label}…"));
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    let git = GitExecutable::discover().map_err(|error| error.to_string())?;
                    command(&git, &worker_repository).map_err(|error| format!("{error:?}"))
                })
                .await;
            let _ = this.update(cx, |app, cx| {
                app.mutation_in_flight = false;
                match result {
                    Ok(()) => {
                        app.set_activity(format!("{label} complete."));
                        app.load_working_copy(repository.clone(), cx);
                        Self::load_refs(repository, cx);
                    }
                    Err(error) => app.set_activity(git_failure_message(&label, &error)),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn toggle_ref_group(&mut self, key: String, cx: &mut Context<Self>) {
        if !self.expanded_ref_groups.remove(&key) {
            self.expanded_ref_groups.insert(key);
        }
        if self
            .store
            .save_expanded_ref_groups(self.expanded_ref_groups.iter().cloned().collect())
            .is_err()
        {
            self.set_activity("Ref group expansion could not be saved.");
        }
        cx.notify();
    }

    fn select_ref_context(&mut self, context: &RefContext, cx: &mut Context<Self>) {
        self.close_ref_context_menu(cx);
        match context {
            RefContext::LocalBranch(name)
            | RefContext::RemoteBranch(name)
            | RefContext::Tag(name) => {
                self.show_ref_history(name.clone(), cx);
            }
            RefContext::Remote(_) => {
                self.show_remotes(cx);
            }
        }
    }

    /// Double-click a sidebar branch to check it out.
    pub(crate) fn activate_ref_from_double_click(
        &mut self,
        context: &RefContext,
        cx: &mut Context<Self>,
    ) {
        self.close_ref_context_menu(cx);
        match context {
            RefContext::LocalBranch(name) => {
                if self.head_branch_name().as_deref() == Some(name.as_str()) {
                    self.set_activity(format!("{name} is already checked out."));
                    cx.notify();
                    return;
                }
                self.checkout_branch(name.clone(), cx);
            }
            RefContext::RemoteBranch(name) => {
                self.checkout_remote_tracking_branch(name.clone(), cx);
            }
            RefContext::Tag(name) => {
                self.show_ref_history(name.clone(), cx);
                self.set_activity("Tags open History; check out a branch to switch.");
                cx.notify();
            }
            RefContext::Remote(_) => {
                self.show_remotes(cx);
            }
        }
    }

    fn checkout_remote_tracking_branch(&mut self, remote_branch: String, cx: &mut Context<Self>) {
        let Some((_remote, short)) = remote_branch.split_once('/') else {
            self.set_activity(format!(
                "Remote branch {remote_branch} is missing a remote prefix."
            ));
            cx.notify();
            return;
        };
        let short = short.to_owned();
        let local_exists = self
            .refs
            .local_branches
            .iter()
            .any(|branch| String::from_utf8_lossy(&branch.name.0) == short);
        if local_exists {
            if self.head_branch_name().as_deref() == Some(short.as_str()) {
                self.set_activity(format!("{short} is already checked out."));
                cx.notify();
                return;
            }
            self.checkout_branch(short, cx);
            return;
        }
        self.run_branch_command(
            format!("Checking out {short} from {remote_branch}"),
            move |git, repository| git.checkout_tracking_branch(repository, &remote_branch),
            cx,
        );
    }

    pub(crate) fn open_ref_context_menu(
        &mut self,
        context: RefContext,
        position: (f32, f32),
        cx: &mut Context<Self>,
    ) {
        self.close_commit_context_menu(cx);
        self.ref_context = Some(context);
        self.ref_context_menu_position = Some(position);
        self.ref_context_submenu = None;
        cx.notify();
    }

    pub(crate) fn close_ref_context_menu(&mut self, cx: &mut Context<Self>) {
        let closed = self.ref_context.take().is_some()
            || self.ref_context_menu_position.take().is_some()
            || self.ref_context_submenu.take().is_some();
        if closed {
            cx.notify();
        }
    }

    pub(crate) fn open_commit_context_menu(
        &mut self,
        index: usize,
        repository: WorktreeRepository,
        position: (f32, f32),
        cx: &mut Context<Self>,
    ) {
        let Some(commit) = self.history.get(index).cloned() else {
            return;
        };
        self.close_ref_context_menu(cx);
        self.select_history_commit(index, repository, cx);
        let short_oid = commit.oid.chars().take(8).collect();
        let subject = String::from_utf8_lossy(&commit.subject).into_owned();
        let is_head = self
            .head_commit_oid()
            .is_some_and(|head| head == commit.oid);
        self.commit_context = Some(CommitContext {
            oid: commit.oid,
            short_oid,
            index,
            subject,
            is_head,
        });
        self.commit_context_menu_position = Some(position);
        cx.notify();
    }

    pub(crate) fn close_commit_context_menu(&mut self, cx: &mut Context<Self>) {
        let closed = self.commit_context.take().is_some()
            || self.commit_context_menu_position.take().is_some();
        if closed {
            cx.notify();
        }
    }

    pub(crate) fn history_is_head_branch_scope(&self) -> bool {
        let Some(head) = self.head_branch_name() else {
            return false;
        };
        match &self.history_reference {
            HistoryReference::Current => true,
            HistoryReference::Named(name) => name == &head,
            HistoryReference::All => false,
        }
    }

    fn head_commit_oid(&self) -> Option<String> {
        self.working_copy.as_ref().and_then(|status| {
            status
                .branch
                .oid
                .as_ref()
                .and_then(|bytes| String::from_utf8(bytes.clone()).ok())
        })
    }

    pub(crate) fn copy_commit_hash(&mut self, oid: &str, cx: &mut Context<Self>) {
        let short = oid.chars().take(8).collect::<String>();
        let text = format!("{short}\n{oid}");
        cx.write_to_clipboard(ClipboardItem::new_string(text));
        self.set_activity(format!("Copied commit hash {short}."));
        cx.notify();
    }

    pub(crate) fn copy_commit_info(&mut self, index: usize, oid: &str, cx: &mut Context<Self>) {
        let Some(commit) = self.history.get(index) else {
            self.copy_commit_hash(oid, cx);
            return;
        };
        let author = String::from_utf8_lossy(&commit.author.name);
        let email = String::from_utf8_lossy(&commit.author.email);
        let subject = String::from_utf8_lossy(&commit.subject);
        let when = crate::views::components::short_calendar_date(commit.author.timestamp);
        let text =
            format!("Author: {author} <{email}>\nDate: {when}\nSubject: {subject}\nCommit: {oid}");
        cx.write_to_clipboard(ClipboardItem::new_string(text));
        self.set_activity("Copied commit info.");
        cx.notify();
    }

    pub(crate) fn reveal_history_commit(&mut self, index: usize, cx: &mut Context<Self>) {
        let ShellState::Repository(repository) = &self.state else {
            return;
        };
        let repository = repository.clone();
        self.navigate_to(RepositoryView::History, cx);
        self.select_history_commit(index, repository, cx);
    }

    pub(crate) fn checkout_detached_commit(&mut self, oid: &str, cx: &mut Context<Self>) {
        let short = oid.chars().take(8).collect::<String>();
        let oid = oid.to_owned();
        self.run_worktree_mutation(
            format!("Checking out {short} (detached)"),
            move |git, repository| git.checkout_detached(repository, &oid),
            cx,
        );
    }

    pub(crate) fn prompt_reset_head_to(&mut self, oid: &str, cx: &mut Context<Self>) {
        self.begin_choice_prompt(
            ChoicePromptKind::ResetMode {
                oid: oid.to_owned(),
            },
            cx,
        );
    }

    pub(crate) fn prompt_revert_commit(&mut self, oid: &str, cx: &mut Context<Self>) {
        self.confirm_dialog = Some(AppConfirmDialog::RevertCommit {
            oid: oid.to_owned(),
        });
        cx.notify();
    }

    pub(crate) fn prompt_rebase_onto_commit(&mut self, oid: &str, cx: &mut Context<Self>) {
        self.begin_text_prompt(TextPromptKind::RebaseOnto, oid, cx);
    }

    pub(crate) fn prompt_drop_history_commit(&mut self, oid: &str, cx: &mut Context<Self>) {
        self.confirm_dialog = Some(AppConfirmDialog::DropCommit {
            oid: oid.to_owned(),
        });
        cx.notify();
    }

    fn run_reset_to(&mut self, oid: &str, mode: ResetMode, cx: &mut Context<Self>) {
        let short = oid.chars().take(8).collect::<String>();
        let mode_label = match mode {
            ResetMode::Soft => "soft",
            ResetMode::Mixed => "mixed",
            ResetMode::Hard => "hard",
        };
        let oid = oid.to_owned();
        self.run_worktree_mutation(
            format!("Reset {mode_label} to {short}"),
            move |git, repository| git.reset_to(repository, &oid, mode),
            cx,
        );
    }

    pub(crate) fn save_commit_patch(&mut self, oid: &str, cx: &mut Context<Self>) {
        let ShellState::Repository(repository) = &self.state else {
            return;
        };
        let repository = repository.clone();
        let oid = oid.to_owned();
        let short = oid.chars().take(8).collect::<String>();
        cx.spawn(async move |this, cx| {
            let destination = cx
                .background_spawn(async move {
                    let output = Command::new("osascript")
                        .args([
                            "-e",
                            "POSIX path of (choose folder with prompt \"Save patch to folder\")",
                        ])
                        .output()
                        .ok()
                        .filter(|output| output.status.success())?;
                    let path = String::from_utf8_lossy(&output.stdout).trim().to_owned();
                    (!path.is_empty()).then(|| PathBuf::from(path))
                })
                .await;
            let Some(destination) = destination else {
                return;
            };
            let result = cx
                .background_spawn({
                    let repository = repository.clone();
                    let oid = oid.clone();
                    let destination = destination.clone();
                    async move {
                        let git = GitExecutable::discover().map_err(|error| error.to_string())?;
                        git.format_patch_to_dir(&repository, &oid, &destination)
                            .map_err(|error| format!("{error:?}"))
                    }
                })
                .await;
            let _ = this.update(cx, |app, cx| {
                match result {
                    Ok(()) => app.set_activity(format!(
                        "Saved patch for {short} to {}.",
                        destination.display()
                    )),
                    Err(error) => app.set_activity(format!("Save patch failed: {error}")),
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn open_ref_context_submenu(
        &mut self,
        submenu: RefContextSubmenu,
        cx: &mut Context<Self>,
    ) {
        if self.ref_context_submenu.as_ref() == Some(&submenu) {
            self.ref_context_submenu = None;
        } else {
            self.ref_context_submenu = Some(submenu);
        }
        cx.notify();
    }

    fn show_ref_history(&mut self, reference: String, cx: &mut Context<Self>) {
        let ShellState::Repository(repository) = &self.state else {
            return;
        };
        let repository = repository.clone();
        self.navigate_to(RepositoryView::History, cx);
        self.change_history_reference(HistoryReference::Named(reference), repository, cx);
    }

    fn prompt_history_search(&mut self, cx: &mut Context<Self>) {
        self.begin_text_prompt(
            TextPromptKind::HistorySearch,
            self.history_search.clone(),
            cx,
        );
    }

    fn prompt_history_reference(&mut self, cx: &mut Context<Self>) {
        self.begin_text_prompt(TextPromptKind::HistoryReference, "", cx);
    }

    fn copy_selected_history_oid(&mut self, cx: &mut Context<Self>) {
        if let Some(commit) = self
            .selected_history
            .and_then(|index| self.history.get(index))
        {
            cx.write_to_clipboard(ClipboardItem::new_string(commit.oid.clone()));
            self.set_activity("Commit OID copied.");
            cx.notify();
        }
    }

    fn reveal_history_head(&mut self, cx: &mut Context<Self>) {
        let Some(oid) = self
            .working_copy
            .as_ref()
            .and_then(|status| status.branch.oid.as_ref())
            .and_then(|oid| std::str::from_utf8(oid).ok())
        else {
            return;
        };
        self.selected_history = self.history.iter().position(|commit| commit.oid == oid);
        cx.notify();
    }

    fn select_history_commit(
        &mut self,
        index: usize,
        repository: WorktreeRepository,
        cx: &mut Context<Self>,
    ) {
        let Some(commit) = self.history.get(index) else {
            return;
        };
        let oid = commit.oid.clone();
        let worker_oid = oid.clone();
        self.selected_history = Some(index);
        self.history_selection_token = self.history_selection_token.wrapping_add(1);
        let selection_token = self.history_selection_token;
        self.history_paths.clear();
        self.history_diff = None;
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    let git = GitExecutable::discover().map_err(|error| error.to_string())?;
                    Ok::<_, String>((
                        git.commit_paths(&repository, &worker_oid)
                            .map_err(|error| format!("{error:?}"))?,
                        git.commit_diff(&repository, &worker_oid)
                            .map_err(|error| format!("{error:?}"))?,
                    ))
                })
                .await;
            let _ = this.update(cx, |app, cx| {
                if app.history_selection_token == selection_token
                    && app.selected_history == Some(index)
                    && app
                        .history
                        .get(index)
                        .is_some_and(|commit| commit.oid == oid)
                {
                    if let Ok((paths, diff)) = result {
                        app.history_paths = paths;
                        app.history_diff = Some(diff);
                    }
                    cx.notify();
                }
            });
        })
        .detach();
        cx.notify();
    }

    fn load_author_identity(repository: WorktreeRepository, cx: &mut Context<Self>) {
        let root = repository.worktree_root.clone();
        cx.spawn(async move |this, cx| {
            let identity = cx
                .background_spawn(async move {
                    GitExecutable::discover()
                        .ok()?
                        .author_identity(&repository)
                        .ok()
                        .flatten()
                        .map(|identity| format!("{} <{}>", identity.name, identity.email))
                })
                .await;
            let _ = this.update(cx, |app, cx| {
                if matches!(&app.state, ShellState::Repository(current) if current.worktree_root == root) {
                    app.author_identity = identity.unwrap_or_else(|| {
                        "Author identity missing: set user.name and user.email in Git.".into()
                    });
                    cx.notify();
                }
            });
        })
        .detach();
    }

    fn schedule_poll(repository: WorktreeRepository, cx: &mut Context<Self>) {
        let root = repository.worktree_root.clone();
        cx.spawn(async move |this, cx| {
            cx.background_spawn(async move {
                std::thread::sleep(Duration::from_secs(2));
            })
            .await;
            let _ = this.update(cx, |app, cx| {
                let ShellState::Repository(current) = &app.state else {
                    return;
                };
                if current.worktree_root == root {
                    if !repository_is_available(&repository) {
                        app.stop_watcher();
                        app.state = ShellState::Error(repository_unavailable_message(&repository));
                        app.set_activity(
                            "Repository is no longer available. Choose it again when it returns.",
                        );
                        cx.notify();
                        return;
                    }
                    let changed = app
                        .watch_events
                        .as_ref()
                        .is_none_or(|events| events.try_iter().next().is_some());
                    if changed {
                        app.load_working_copy(repository.clone(), cx);
                    }
                    Self::schedule_poll(repository, cx);
                }
            });
        })
        .detach();
    }

    fn start_watcher(&mut self, repository: &WorktreeRepository) {
        self.stop_watcher();
        let (sender, receiver) = mpsc::channel();
        let Ok(mut watcher) = notify::recommended_watcher(move |_| {
            let _ = sender.send(());
        }) else {
            return;
        };
        if watcher
            .watch(&repository.worktree_root, RecursiveMode::Recursive)
            .is_ok()
        {
            self.watcher = Some(watcher);
            self.watch_events = Some(receiver);
        }
    }

    fn stop_watcher(&mut self) {
        self.watcher = None;
        self.watch_events = None;
    }

    fn select_status_path(
        &mut self,
        path: GitPath,
        additive: bool,
        shift: bool,
        staged: bool,
        cx: &mut Context<Self>,
    ) {
        let path_for_index = path.clone();
        let visible = self.visible_status_paths();
        let all_visible_selected = !visible.is_empty()
            && visible
                .iter()
                .all(|candidate| self.selected_paths.contains(candidate));

        if !additive && !shift {
            // Clicking the row that cleared a full selection restores it; every other row falls
            // through to plain single selection so one file can always be picked.
            if self.file_list_select_all_toggle.as_ref() == Some(&path) && visible.contains(&path) {
                self.last_selected_path_index = visible.iter().position(|p| p == &path);
                self.selected_paths = visible;
                self.file_list_select_all_toggle = None;
                self.selected_diff = None;
                self.loaded_diff = None;
                cx.notify();
                return;
            }
            if visible.len() > 1 && all_visible_selected && self.selected_paths.contains(&path) {
                self.selected_paths.clear();
                self.file_list_select_all_toggle = Some(path);
                self.selected_diff = None;
                self.loaded_diff = None;
                self.last_selected_path_index = None;
                cx.notify();
                return;
            }
        }
        self.file_list_select_all_toggle = None;

        if shift {
            if let Some(last_index) = self.last_selected_path_index {
                if let Some(current_index) = visible.iter().position(|p| p == &path) {
                    let start = last_index.min(current_index);
                    let end = last_index.max(current_index);
                    for p in visible.iter().skip(start).take(end + 1 - start) {
                        if !self.selected_paths.contains(p) {
                            self.selected_paths.push(p.clone());
                        }
                    }
                }
            } else if let Some(index) = self
                .selected_paths
                .iter()
                .position(|selected| selected == &path)
            {
                self.selected_paths.remove(index);
            } else {
                self.selected_paths.push(path.clone());
            }
        } else if additive {
            if let Some(index) = self
                .selected_paths
                .iter()
                .position(|selected| selected == &path)
            {
                self.selected_paths.remove(index);
            } else {
                self.selected_paths.push(path.clone());
            }
        } else {
            self.selected_paths = vec![path];
        }
        if !additive
            && !shift
            && let ShellState::Repository(repository) = &self.state
        {
            self.selected_diff = Some((self.selected_paths[0].clone(), staged));
            Self::load_diff(
                repository.clone(),
                self.selected_paths[0].clone(),
                staged,
                git_cli::MAX_DISPLAY_DIFF_BYTES,
                cx,
            );
        }
        self.last_selected_path_index = visible.iter().position(|p| p == &path_for_index);
        cx.notify();
    }

    fn load_diff(
        repository: WorktreeRepository,
        path: GitPath,
        staged: bool,
        limit: usize,
        cx: &mut Context<Self>,
    ) {
        let stats_path = path.clone();
        cx.spawn(async move |this, cx| {
            let diff = cx
                .background_spawn(async move {
                    GitExecutable::discover()
                        .map_err(|error| error.to_string())?
                        .file_diff_with_limit(&repository, &path, staged, limit)
                        .map_err(|error| format!("{error:?}"))
                })
                .await;
            let _ = this.update(cx, |app, cx| {
                if let Ok(diff) = diff {
                    let mut additions = 0;
                    let mut deletions = 0;
                    for file in &diff.diff.files {
                        for hunk in &file.hunks {
                            for line in &hunk.lines {
                                match line.kind {
                                    git_domain::DiffLineKind::Addition => additions += 1,
                                    git_domain::DiffLineKind::Removal => deletions += 1,
                                    git_domain::DiffLineKind::Context => {}
                                }
                            }
                        }
                    }
                    app.file_diff_stats
                        .insert(stats_path, (additions, deletions));
                    app.loaded_diff = Some(diff);
                    app.selected_diff_lines.clear();
                    app.pending_line_discard = None;
                    app.pending_hunk_discard = None;
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn show_status_context_menu(&mut self, path: GitPath, cx: &mut Context<Self>) {
        self.context_path = Some(path);
        cx.notify();
    }

    fn toggle_path_staged(
        &mut self,
        path: &GitPath,
        currently_staged: bool,
        cx: &mut Context<Self>,
    ) {
        if self.mutation_in_flight {
            self.set_activity("Another Git operation is still running. Try again in a moment.");
            cx.notify();
            return;
        }
        if !self.selected_paths.contains(path) {
            self.selected_paths = vec![path.clone()];
        }
        let paths = self.selected_paths.clone();
        let staged_flags: Vec<bool> = paths.iter().map(|path| self.path_is_staged(path)).collect();
        let operation = if should_stage_selection(&staged_flags, currently_staged) {
            Mutation::StageSelected
        } else {
            Mutation::UnstageSelected
        };
        self.run_mutation(operation, paths, true, cx);
    }

    fn path_is_staged(&self, path: &GitPath) -> bool {
        self.working_copy.as_ref().is_some_and(|status| {
            status
                .entries
                .iter()
                .any(|entry| status_path(entry) == path && entry_is_staged(entry))
        })
    }

    pub(crate) fn toggle_worktree_show_all(&mut self, cx: &mut Context<Self>) {
        self.worktree_show_all_files = !self.worktree_show_all_files;
        if self.worktree_show_all_files {
            let ShellState::Repository(repository) = &self.state else {
                self.worktree_show_all_files = false;
                return;
            };
            let repository = repository.clone();
            let worker_repository = repository.clone();
            cx.spawn(async move |this, cx| {
                let files = cx
                    .background_spawn(async move {
                        let git = GitExecutable::discover().map_err(|error| error.to_string())?;
                        git.tracked_files(&worker_repository)
                            .map_err(|error| format!("{error:?}"))
                    })
                    .await;
                let _ = this.update(cx, |app, cx| {
                    if app.worktree_show_all_files {
                        match files {
                            Ok(files) => app.tracked_files = files,
                            Err(error) => {
                                app.set_activity(git_failure_message("List all files", &error));
                            }
                        }
                    }
                    cx.notify();
                });
            })
            .detach();
        } else {
            self.tracked_files.clear();
        }
        cx.notify();
    }

    fn load_full_diff(&mut self, cx: &mut Context<Self>) {
        let Some((path, staged)) = self.selected_diff.clone() else {
            return;
        };
        let ShellState::Repository(repository) = &self.state else {
            return;
        };
        let repository = repository.clone();
        self.set_activity("Loading full diff…");
        Self::load_diff(repository, path, staged, usize::MAX, cx);
        cx.notify();
    }

    fn stage_diff_hunk(&mut self, hunk_index: usize, cx: &mut Context<Self>) {
        if self.mutation_in_flight {
            return;
        }
        let Some((path, false)) = self.selected_diff.clone() else {
            return;
        };
        let ShellState::Repository(repository) = &self.state else {
            return;
        };
        let repository = repository.clone();
        let worker_repository = repository.clone();
        let worker_path = path.clone();
        self.mutation_in_flight = true;
        self.set_activity("Staging hunk…");
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    GitExecutable::discover()
                        .map_err(|error| error.to_string())?
                        .stage_hunk(&worker_repository, &worker_path, hunk_index)
                        .map_err(|error| format!("{error:?}"))
                })
                .await;
            let _ = this.update(cx, |app, cx| {
                app.mutation_in_flight = false;
                match result {
                    Ok(()) => {
                        app.set_activity("Hunk staged.");
                        app.load_working_copy(repository.clone(), cx);
                        Self::load_diff(
                            repository,
                            path,
                            false,
                            git_cli::MAX_DISPLAY_DIFF_BYTES,
                            cx,
                        );
                    }
                    Err(error) => app.set_activity(git_failure_message("Stage hunk", &error)),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn unstage_diff_hunk(&mut self, hunk_index: usize, cx: &mut Context<Self>) {
        if self.mutation_in_flight {
            return;
        }
        let Some((path, true)) = self.selected_diff.clone() else {
            return;
        };
        let ShellState::Repository(repository) = &self.state else {
            return;
        };
        let repository = repository.clone();
        let worker_repository = repository.clone();
        let worker_path = path.clone();
        self.mutation_in_flight = true;
        self.set_activity("Unstaging hunk…");
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    GitExecutable::discover()
                        .map_err(|error| error.to_string())?
                        .unstage_hunk(&worker_repository, &worker_path, hunk_index)
                        .map_err(|error| format!("{error:?}"))
                })
                .await;
            let _ = this.update(cx, |app, cx| {
                app.mutation_in_flight = false;
                match result {
                    Ok(()) => {
                        app.set_activity("Hunk unstaged.");
                        app.load_working_copy(repository.clone(), cx);
                        Self::load_diff(
                            repository,
                            path,
                            true,
                            git_cli::MAX_DISPLAY_DIFF_BYTES,
                            cx,
                        );
                    }
                    Err(error) => app.set_activity(git_failure_message("Unstage hunk", &error)),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn copy_context_path(&mut self, repository: &WorktreeRepository, cx: &mut Context<Self>) {
        let Some(path) = self.context_path.as_ref() else {
            return;
        };
        let path = repository
            .worktree_root
            .join(OsString::from_vec(path.0.clone()));
        cx.write_to_clipboard(ClipboardItem::new_string(path.display().to_string()));
        self.set_activity("Path copied.");
        cx.notify();
    }

    fn open_context_path(
        &mut self,
        repository: &WorktreeRepository,
        reveal: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(path) = self.context_path.as_ref() else {
            return;
        };
        let path = repository
            .worktree_root
            .join(OsString::from_vec(path.0.clone()));
        self.set_activity(if reveal {
            "Revealing file in Finder…"
        } else {
            "Opening file…"
        });
        cx.background_spawn(async move {
            let mut command = Command::new("open");
            if reveal {
                command.arg("-R");
            }
            let _ = command.arg(path).status();
        })
        .detach();
        cx.notify();
    }

    fn mutate(&mut self, operation: Mutation, cx: &mut Context<Self>) {
        if self.mutation_in_flight {
            return;
        }
        let ShellState::Repository(_) = &self.state else {
            return;
        };
        let paths = self.selected_paths.clone();
        if operation.needs_paths() && paths.is_empty() {
            self.set_activity("Select at least one file first.");
            cx.notify();
            return;
        }
        if operation == Mutation::DiscardSelected && self.pending_discard.as_ref() != Some(&paths) {
            self.pending_discard = Some(paths);
            self.set_activity("Review the discard consequences, then confirm.");
            cx.notify();
            return;
        }
        self.run_mutation(operation, paths, false, cx);
    }

    fn confirm_discard(&mut self, cx: &mut Context<Self>) {
        let Some(paths) = self.pending_discard.take() else {
            return;
        };
        self.run_mutation(Mutation::DiscardSelected, paths, false, cx);
    }

    fn toggle_diff_line(&mut self, hunk_index: usize, line_index: usize, cx: &mut Context<Self>) {
        if self.mutation_in_flight {
            return;
        }
        let can_edit_lines = self
            .loaded_diff
            .as_ref()
            .and_then(|loaded| loaded.diff.files.first())
            .and_then(|file| file.hunks.get(hunk_index))
            .and_then(|hunk| hunk.lines.get(line_index))
            .is_some_and(|line| {
                matches!(
                    line.kind,
                    git_domain::DiffLineKind::Addition | git_domain::DiffLineKind::Removal
                )
            })
            && matches!(self.selected_diff, Some((_, false)));
        if !can_edit_lines {
            return;
        }
        if let Some(index) = self
            .selected_diff_lines
            .iter()
            .position(|&(hunk, line)| hunk == hunk_index && line == line_index)
        {
            self.selected_diff_lines.remove(index);
        } else {
            self.selected_diff_lines.push((hunk_index, line_index));
        }
        cx.notify();
    }

    fn stage_selected_diff_lines(&mut self, cx: &mut Context<Self>) {
        if self.mutation_in_flight || self.selected_diff_lines.is_empty() {
            return;
        }
        let Some((path, false)) = self.selected_diff.clone() else {
            return;
        };
        let ShellState::Repository(repository) = &self.state else {
            return;
        };
        let repository = repository.clone();
        let worker_repository = repository.clone();
        let worker_path = path.clone();
        let selection = self.selected_diff_lines.clone();
        self.mutation_in_flight = true;
        self.set_activity("Staging selected lines…");
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    GitExecutable::discover()
                        .map_err(|error| error.to_string())?
                        .stage_lines(&worker_repository, &worker_path, &selection)
                        .map_err(|error| format!("{error:?}"))
                })
                .await;
            let _ = this.update(cx, |app, cx| {
                app.mutation_in_flight = false;
                match result {
                    Ok(()) => {
                        app.set_activity("Selected lines staged.");
                        app.load_working_copy(repository.clone(), cx);
                        Self::load_diff(
                            repository,
                            path,
                            false,
                            git_cli::MAX_DISPLAY_DIFF_BYTES,
                            cx,
                        );
                    }
                    Err(error) => {
                        app.set_activity(git_failure_message("Stage selected lines", &error));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn request_line_discard(&mut self, cx: &mut Context<Self>) {
        if self.mutation_in_flight || self.selected_diff_lines.is_empty() {
            return;
        }
        let Some((path, false)) = self.selected_diff.clone() else {
            return;
        };
        self.pending_line_discard = Some((path, self.selected_diff_lines.clone()));
        self.set_activity("Review the line discard consequences, then confirm.");
        cx.notify();
    }

    fn cancel_line_discard(&mut self, cx: &mut Context<Self>) {
        self.pending_line_discard = None;
        self.set_activity("Line discard cancelled.");
        cx.notify();
    }

    fn confirm_line_discard(&mut self, cx: &mut Context<Self>) {
        let Some((path, selection)) = self.pending_line_discard.take() else {
            return;
        };
        if self.mutation_in_flight {
            return;
        }
        let ShellState::Repository(repository) = &self.state else {
            return;
        };
        let repository = repository.clone();
        let worker_repository = repository.clone();
        let worker_path = path.clone();
        self.mutation_in_flight = true;
        self.set_activity("Discarding selected lines…");
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    GitExecutable::discover()
                        .map_err(|error| error.to_string())?
                        .discard_lines(&worker_repository, &worker_path, &selection)
                        .map_err(|error| format!("{error:?}"))
                })
                .await;
            let _ = this.update(cx, |app, cx| {
                app.mutation_in_flight = false;
                app.selected_diff_lines.clear();
                match result {
                    Ok(()) => {
                        app.set_activity("Selected lines discarded.");
                        app.load_working_copy(repository.clone(), cx);
                        Self::load_diff(
                            repository,
                            path,
                            false,
                            git_cli::MAX_DISPLAY_DIFF_BYTES,
                            cx,
                        );
                    }
                    Err(error) => {
                        app.set_activity(git_failure_message("Discard selected lines", &error));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn request_hunk_discard(&mut self, hunk_index: usize, cx: &mut Context<Self>) {
        if self.mutation_in_flight {
            return;
        }
        let Some((path, false)) = self.selected_diff.clone() else {
            return;
        };
        self.pending_hunk_discard = Some((path, hunk_index));
        self.set_activity("Review the hunk discard consequences, then confirm.");
        cx.notify();
    }

    fn cancel_hunk_discard(&mut self, cx: &mut Context<Self>) {
        self.pending_hunk_discard = None;
        self.set_activity("Hunk discard cancelled.");
        cx.notify();
    }

    fn confirm_hunk_discard(&mut self, cx: &mut Context<Self>) {
        let Some((path, hunk_index)) = self.pending_hunk_discard.take() else {
            return;
        };
        if self.mutation_in_flight {
            return;
        }
        let ShellState::Repository(repository) = &self.state else {
            return;
        };
        let repository = repository.clone();
        let worker_repository = repository.clone();
        let worker_path = path.clone();
        self.mutation_in_flight = true;
        self.set_activity("Discarding hunk…");
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    GitExecutable::discover()
                        .map_err(|error| error.to_string())?
                        .discard_hunk(&worker_repository, &worker_path, hunk_index)
                        .map_err(|error| format!("{error:?}"))
                })
                .await;
            let _ = this.update(cx, |app, cx| {
                app.mutation_in_flight = false;
                match result {
                    Ok(()) => {
                        app.set_activity("Hunk discarded.");
                        app.load_working_copy(repository.clone(), cx);
                        Self::load_diff(
                            repository,
                            path,
                            false,
                            git_cli::MAX_DISPLAY_DIFF_BYTES,
                            cx,
                        );
                    }
                    Err(error) => app.set_activity(git_failure_message("Discard hunk", &error)),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn request_operation_abort(&mut self, cx: &mut Context<Self>) {
        if self.mutation_in_flight {
            return;
        }
        let Some(operation) = self
            .working_copy
            .as_ref()
            .map(|status| status.operation.clone())
        else {
            return;
        };
        if operation == git_domain::InProgressOperation::None {
            return;
        }
        self.pending_operation_action = Some(OperationAction::Abort);
        self.set_activity("Aborting discards the paused operation and returns to its start state.");
        cx.notify();
    }

    fn request_operation_continue(&mut self, cx: &mut Context<Self>) {
        if self.mutation_in_flight {
            return;
        }
        let Some(operation) = self
            .working_copy
            .as_ref()
            .map(|status| status.operation.clone())
        else {
            return;
        };
        if operation == git_domain::InProgressOperation::None {
            return;
        }
        self.pending_operation_action = Some(OperationAction::Continue);
        self.set_activity(
            "Resolve conflicts, stage the resolved files, then confirm to continue the operation.",
        );
        cx.notify();
    }

    fn cancel_operation_action(&mut self, cx: &mut Context<Self>) {
        self.pending_operation_action = None;
        self.set_activity("Operation action cancelled.");
        cx.notify();
    }

    fn confirm_operation_action(&mut self, cx: &mut Context<Self>) {
        let Some(action) = self.pending_operation_action.take() else {
            return;
        };
        if self.mutation_in_flight {
            return;
        }
        let Some(operation) = self
            .working_copy
            .as_ref()
            .map(|status| status.operation.clone())
        else {
            return;
        };
        let ShellState::Repository(repository) = &self.state else {
            return;
        };
        let repository = repository.clone();
        let worker_repository = repository.clone();
        let repository_path = repository.worktree_root.clone();
        let journal = RecoveryJournalStore::new(recovery_journal_path());
        self.mutation_in_flight = true;
        self.set_activity(match action {
            OperationAction::Abort => "Aborting operation…",
            OperationAction::Continue => "Continuing operation…",
        });
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    let git = GitExecutable::discover().map_err(|error| error.to_string())?;
                    let snapshot = git.recovery_snapshot(&worker_repository).map_err(|error| {
                        format!("Could not record the pre-operation state: {error:?}")
                    })?;
                    let journal_warning = journal
                        .record_entry(repository_path, snapshot)
                        .err()
                        .map(|error| format!("Pre-operation refs were not journaled: {error}"));
                    let outcome = match (action, &operation) {
                        (OperationAction::Abort, git_domain::InProgressOperation::Merge { .. }) => {
                            git.abort_merge(&worker_repository)
                        }
                        (
                            OperationAction::Abort,
                            git_domain::InProgressOperation::CherryPick { .. },
                        ) => git.abort_cherry_pick(&worker_repository),
                        (
                            OperationAction::Abort,
                            git_domain::InProgressOperation::Revert { .. },
                        ) => git.abort_revert(&worker_repository),
                        (OperationAction::Abort, git_domain::InProgressOperation::Rebase) => {
                            git.abort_rebase(&worker_repository)
                        }
                        (OperationAction::Continue, operation) => {
                            git.continue_operation(&worker_repository, operation)
                        }
                        (OperationAction::Abort, git_domain::InProgressOperation::None) => {
                            Err(GitStatusError::NoOperationInProgress)
                        }
                    };
                    outcome
                        .map_err(|error| format!("{error:?}"))
                        .map(|()| journal_warning)
                })
                .await;
            let _ = this.update(cx, |app, cx| {
                app.mutation_in_flight = false;
                match result {
                    Ok(journal_warning) => {
                        let base = match action {
                            OperationAction::Abort => "Operation aborted.",
                            OperationAction::Continue => "Operation continued.",
                        };
                        app.set_activity(match journal_warning {
                            Some(warning) => format!("{base} {warning}"),
                            None => base.into(),
                        });
                        app.load_working_copy(repository, cx);
                    }
                    Err(error) => {
                        let label = match action {
                            OperationAction::Abort => "Abort operation",
                            OperationAction::Continue => "Continue operation",
                        };
                        app.set_activity(git_failure_message(label, &error));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn show_stashes(&mut self, repository: WorktreeRepository, cx: &mut Context<Self>) {
        self.navigate_to(RepositoryView::Stashes, cx);
        self.load_stashes(repository, cx);
    }

    fn load_stashes(&mut self, repository: WorktreeRepository, cx: &mut Context<Self>) {
        let root = repository.worktree_root.clone();
        self.stashes_load_token = self.stashes_load_token.wrapping_add(1);
        let load_token = self.stashes_load_token;
        self.set_activity("Loading stashes…");
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    GitExecutable::discover()
                        .map_err(|error| error.to_string())?
                        .stash_list(&repository)
                        .map_err(|error| format!("{error:?}"))
                })
                .await;
            let _ = this.update(cx, |app, cx| {
                if !matches!(&app.state, ShellState::Repository(current) if current.worktree_root == root)
                    || app.stashes_load_token != load_token
                {
                    return;
                }
                match result {
                    Ok(stashes) => {
                        app.stashes = stashes;
                        app.selected_stash = None;
                        app.selected_stash_paths.clear();
                        app.selected_stash_diff = None;
                        app.stash_selection_token =
                            app.stash_selection_token.wrapping_add(1);
                        app.set_activity(format!("Loaded {} stash entr(ies).", app.stashes.len()));
                    }
                    Err(error) => app.set_activity(git_failure_message("List stashes", &error)),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn move_stash_selection(&mut self, delta: isize, cx: &mut Context<Self>) {
        if self.repository_view != RepositoryView::Stashes || self.stashes.is_empty() {
            return;
        }
        let current = self.selected_stash.unwrap_or(0);
        let index = if delta.is_negative() {
            current.saturating_sub(delta.unsigned_abs())
        } else {
            current
                .saturating_add(delta.unsigned_abs())
                .min(self.stashes.len() - 1)
        };
        self.select_stash(index, cx);
    }

    fn selected_stash(&self) -> Option<(String, String)> {
        self.selected_stash.and_then(|index| {
            self.stashes.get(index).map(|entry| {
                (
                    entry.reference.clone(),
                    String::from_utf8_lossy(&entry.subject).into_owned(),
                )
            })
        })
    }

    pub(crate) fn open_stash_save_dialog(
        &mut self,
        include_untracked: bool,
        paths: Vec<GitPath>,
        cx: &mut Context<Self>,
    ) {
        if !matches!(self.state, ShellState::Repository(_)) {
            self.set_activity("Open a repository before saving a stash.");
            cx.notify();
            return;
        }
        self.begin_text_prompt(
            TextPromptKind::CreateStash {
                include_untracked,
                paths,
            },
            "",
            cx,
        );
    }

    pub(crate) fn toggle_create_stash_include_untracked(&mut self, cx: &mut Context<Self>) {
        if let Some(TextPromptKind::CreateStash {
            include_untracked, ..
        }) = self.pending_text_prompt.as_mut()
        {
            *include_untracked = !*include_untracked;
            cx.notify();
        }
    }

    pub(crate) fn open_stash_apply_dialog_for_selection(&mut self, cx: &mut Context<Self>) {
        let Some((reference, _)) = self.selected_stash() else {
            self.set_activity("Select a stash first.");
            cx.notify();
            return;
        };
        self.stash_apply_dialog = Some(StashApplyDialog {
            reference,
            delete_after: false,
            restore_index: false,
        });
        cx.notify();
    }

    pub(crate) fn open_apply_latest_stash_dialog(&mut self, cx: &mut Context<Self>) {
        if !matches!(self.state, ShellState::Repository(_)) {
            self.set_activity("Open a repository before applying a stash.");
            cx.notify();
            return;
        }
        self.stash_apply_dialog = Some(StashApplyDialog {
            reference: "stash@{0}".into(),
            delete_after: false,
            restore_index: false,
        });
        cx.notify();
    }

    pub(crate) fn close_stash_apply_dialog(&mut self, cx: &mut Context<Self>) {
        self.stash_apply_dialog = None;
        cx.notify();
    }

    pub(crate) fn toggle_stash_apply_delete_after(&mut self, cx: &mut Context<Self>) {
        if let Some(dialog) = self.stash_apply_dialog.as_mut() {
            dialog.delete_after = !dialog.delete_after;
            cx.notify();
        }
    }

    pub(crate) fn toggle_stash_apply_restore_index(&mut self, cx: &mut Context<Self>) {
        if let Some(dialog) = self.stash_apply_dialog.as_mut() {
            dialog.restore_index = !dialog.restore_index;
            cx.notify();
        }
    }

    pub(crate) fn confirm_stash_apply_dialog(&mut self, cx: &mut Context<Self>) {
        let Some(dialog) = self.stash_apply_dialog.take() else {
            return;
        };
        self.run_apply_or_pop_stash(
            dialog.reference,
            dialog.delete_after,
            dialog.restore_index,
            cx,
        );
    }

    fn run_apply_or_pop_stash(
        &mut self,
        reference: String,
        delete_after: bool,
        restore_index: bool,
        cx: &mut Context<Self>,
    ) {
        if self.mutation_in_flight {
            return;
        }
        let ShellState::Repository(repository) = &self.state else {
            return;
        };
        let repository = repository.clone();
        let worker_repository = repository.clone();
        let worker_reference = reference.clone();
        self.mutation_in_flight = true;
        let label = if delete_after {
            "Pop stash"
        } else {
            "Apply stash"
        };
        self.set_activity(format!("{label} {reference}…"));
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    let git = GitExecutable::discover().map_err(|error| error.to_string())?;
                    if delete_after {
                        git.pop_stash(&worker_repository, &worker_reference, restore_index)
                    } else {
                        git.apply_stash(&worker_repository, &worker_reference, restore_index)
                    }
                    .map_err(|error| format!("{error:?}"))
                })
                .await;
            let _ = this.update(cx, |app, cx| {
                app.mutation_in_flight = false;
                match result {
                    Ok(()) => {
                        app.set_activity(format!("{label} {reference} complete."));
                        app.load_working_copy(repository.clone(), cx);
                        app.load_stashes(repository, cx);
                    }
                    Err(error) => app.set_activity(git_failure_message(label, &error)),
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn select_stash(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(stash) = self.stashes.get(index).cloned() else {
            return;
        };
        let ShellState::Repository(repository) = &self.state else {
            return;
        };
        let repository = repository.clone();
        let oid = stash.oid.clone();
        self.selected_stash = Some(index);
        self.pending_stash_action_ref = None;
        self.stash_selection_token = self.stash_selection_token.wrapping_add(1);
        let selection_token = self.stash_selection_token;
        self.selected_stash_paths.clear();
        self.selected_stash_diff = None;
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    let git = GitExecutable::discover().map_err(|error| error.to_string())?;
                    Ok::<_, String>((
                        git.commit_paths(&repository, &oid)
                            .map_err(|error| format!("{error:?}"))?,
                        git.commit_diff(&repository, &oid)
                            .map_err(|error| format!("{error:?}"))?,
                    ))
                })
                .await;
            let _ = this.update(cx, |app, cx| {
                if app.stash_selection_token == selection_token
                    && app.selected_stash == Some(index)
                    && app
                        .stashes
                        .get(index)
                        .is_some_and(|entry| entry.oid == stash.oid)
                {
                    if let Ok((paths, diff)) = result {
                        app.selected_stash_paths = paths;
                        app.selected_stash_diff = Some(diff);
                    }
                    cx.notify();
                }
            });
        })
        .detach();
        cx.notify();
    }

    pub(crate) fn request_stash_action(&mut self, action: StashAction, cx: &mut Context<Self>) {
        let Some((reference, subject)) = self.selected_stash() else {
            self.set_activity("Select a stash first.");
            cx.notify();
            return;
        };
        self.pending_stash_action_ref = Some((action, reference, subject));
        self.set_activity(match action {
            StashAction::Pop => "Confirm before removing the stash recovery entry.",
            StashAction::Drop => "Confirm before permanently removing the stash.",
        });
        cx.notify();
    }

    pub(crate) fn cancel_stash_action_ref(&mut self, cx: &mut Context<Self>) {
        self.pending_stash_action_ref = None;
        cx.notify();
    }

    pub(crate) fn confirm_stash_action_ref(&mut self, cx: &mut Context<Self>) {
        let Some((action, reference, _)) = self.pending_stash_action_ref.take() else {
            return;
        };
        match action {
            StashAction::Pop => self.run_apply_or_pop_stash(reference, true, false, cx),
            StashAction::Drop => self.run_drop_stash(reference, cx),
        }
    }

    fn run_drop_stash(&mut self, reference: String, cx: &mut Context<Self>) {
        if self.mutation_in_flight {
            return;
        }
        let ShellState::Repository(repository) = &self.state else {
            return;
        };
        let repository = repository.clone();
        let worker_repository = repository.clone();
        let worker_reference = reference.clone();
        self.mutation_in_flight = true;
        self.set_activity(format!("Drop stash {reference}…"));
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    GitExecutable::discover()
                        .map_err(|error| error.to_string())?
                        .drop_stash(&worker_repository, &worker_reference)
                        .map_err(|error| format!("{error:?}"))
                })
                .await;
            let _ = this.update(cx, |app, cx| {
                app.mutation_in_flight = false;
                match result {
                    Ok(()) => {
                        app.set_activity(format!("Drop stash {reference} complete."));
                        app.load_working_copy(repository.clone(), cx);
                        app.load_stashes(repository, cx);
                    }
                    Err(error) => app.set_activity(git_failure_message("Drop stash", &error)),
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn prompt_branch_from_selected_stash(&mut self, cx: &mut Context<Self>) {
        let Some((reference, _)) = self.selected_stash() else {
            self.set_activity("Select a stash first.");
            cx.notify();
            return;
        };
        self.begin_text_prompt(TextPromptKind::StashBranch { reference }, "", cx);
    }

    fn run_stash_branch(&mut self, branch: String, reference: String, cx: &mut Context<Self>) {
        if self.mutation_in_flight {
            return;
        }
        let ShellState::Repository(repository) = &self.state else {
            return;
        };
        let repository = repository.clone();
        let worker_repository = repository.clone();
        let activity_branch = branch.clone();
        self.mutation_in_flight = true;
        self.set_activity(format!("Creating branch {branch} from {reference}…"));
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    GitExecutable::discover()
                        .map_err(|error| error.to_string())?
                        .stash_branch(&worker_repository, &branch, &reference)
                        .map_err(|error| format!("{error:?}"))
                })
                .await;
            let _ = this.update(cx, |app, cx| {
                app.mutation_in_flight = false;
                match result {
                    Ok(()) => {
                        app.set_activity(format!(
                            "Checked out branch {activity_branch} from stash."
                        ));
                        app.load_working_copy(repository.clone(), cx);
                        app.load_stashes(repository, cx);
                    }
                    Err(error) => {
                        app.set_activity(git_failure_message("Branch from stash", &error));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn show_remotes(&mut self, cx: &mut Context<Self>) {
        self.navigate_to(RepositoryView::Remotes, cx);
        cx.notify();
    }

    fn fetch_remote(&mut self, name: String, cx: &mut Context<Self>) {
        self.run_network_command(
            format!("Fetching {name}"),
            vec!["fetch".into(), "--progress".into(), name.into()],
            cx,
        );
    }

    fn run_create_stash(&mut self, request: CreateStashRequest, cx: &mut Context<Self>) {
        if self.mutation_in_flight {
            return;
        }
        let ShellState::Repository(repository) = &self.state else {
            return;
        };
        let repository = repository.clone();
        let worker_repository = repository.clone();
        self.mutation_in_flight = true;
        self.set_activity("Creating stash…");
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    GitExecutable::discover()
                        .map_err(|error| error.to_string())?
                        .create_stash(&worker_repository, &request)
                        .map_err(|error| format!("{error:?}"))
                })
                .await;
            let _ = this.update(cx, |app, cx| {
                app.mutation_in_flight = false;
                match result {
                    Ok(()) => {
                        app.set_activity("Stash created.");
                        app.load_working_copy(repository.clone(), cx);
                        app.load_stashes(repository, cx);
                    }
                    Err(error) => app.set_activity(git_failure_message("Create stash", &error)),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn edit_commit_subject(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.commit_subject_focused = true;
        self.commit_composer_expanded = true;
        window.focus(&self.commit_subject_input.focus_handle(cx));
        cx.notify();
    }
    pub(crate) fn toggle_commit_amend(&mut self, cx: &mut Context<Self>) {
        if self.commit_amend {
            self.commit_amend = false;
            self.commit_amend_short_oid = None;
            if let Some((subject, body)) = self.commit_pre_amend_draft.take() {
                self.commit_subject = subject;
                self.commit_body = body;
                self.refresh_commit_inputs(cx);
            }
            self.sync_commit_composer_expanded(cx);
            cx.notify();
            return;
        }

        let ShellState::Repository(repository) = &self.state else {
            self.set_activity("Open a repository before amending.");
            cx.notify();
            return;
        };
        let repository = repository.clone();
        self.commit_pre_amend_draft = Some((self.commit_subject.clone(), self.commit_body.clone()));
        self.commit_amend = true;
        self.commit_composer_expanded = true;
        self.set_activity("Loading last commit for amend…");
        cx.notify();
        cx.spawn(async move |this, cx| {
            let summary = cx
                .background_spawn(async move {
                    let git = GitExecutable::discover().map_err(|error| error.to_string())?;
                    git.head_commit_summary(&repository)
                        .map_err(|error| format!("{error:?}"))
                })
                .await;
            let _ = this.update(cx, |app, cx| {
                if !app.commit_amend {
                    return;
                }
                match summary {
                    Ok(summary) => {
                        app.commit_amend_short_oid = Some(summary.short_oid);
                        app.commit_subject = summary.subject;
                        app.commit_body = summary.body;
                        app.refresh_commit_inputs(cx);
                        app.commit_composer_expanded = true;
                        app.set_activity("Amend armed — edit message or stage more changes.");
                    }
                    Err(error) => {
                        app.commit_amend = false;
                        app.commit_amend_short_oid = None;
                        if let Some((subject, body)) = app.commit_pre_amend_draft.take() {
                            app.commit_subject = subject;
                            app.commit_body = body;
                            app.refresh_commit_inputs(cx);
                        }
                        app.set_activity(git_failure_message("Amend", &error));
                    }
                }
                app.sync_commit_composer_expanded(cx);
                cx.notify();
            });
        })
        .detach();
    }
    fn toggle_commit_sign_off(&mut self, cx: &mut Context<Self>) {
        self.commit_sign_off = !self.commit_sign_off;
        if self.commit_sign_off {
            self.commit_composer_expanded = true;
        } else {
            self.sync_commit_composer_expanded(cx);
        }
        cx.notify();
    }

    fn commit_draft(&mut self, cx: &mut Context<Self>) {
        let staged_count = self.status_groups().staged.len();
        // Amend may rewrite HEAD message with zero staged files; normal commit needs staged.
        let can_submit =
            !self.commit_subject.trim().is_empty() && (staged_count > 0 || self.commit_amend);
        if self.mutation_in_flight || !can_submit {
            return;
        }
        let ShellState::Repository(repository) = &self.state else {
            return;
        };
        let repository = repository.clone();
        let worker_repository = repository.clone();
        let amending = self.commit_amend;
        let request = CommitRequest {
            subject: self.commit_subject.clone(),
            body: self.commit_body.clone(),
            amend: amending,
            sign_off: self.commit_sign_off,
        };
        self.mutation_in_flight = true;
        self.set_activity(if amending {
            "Amending…"
        } else {
            "Committing…"
        });
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    let git = GitExecutable::discover().map_err(|error| error.to_string())?;
                    git.commit(&worker_repository, &request)
                        .map_err(|error| format!("{error:?}"))?;
                    let oid = git
                        .head_oid(&worker_repository)
                        .map_err(|error| format!("{error:?}"))?;
                    Ok::<_, String>(oid)
                })
                .await;
            let _ = this.update(cx, |app, cx| {
                app.mutation_in_flight = false;
                match result {
                    Ok(oid) => {
                        app.commit_subject.clear();
                        app.commit_body.clear();
                        app.commit_amend = false;
                        app.commit_amend_short_oid = None;
                        app.commit_pre_amend_draft = None;
                        app.refresh_commit_inputs(cx);
                        app.commit_subject_focused = false;
                        app.commit_body_focused = false;
                        app.sync_commit_composer_expanded(cx);
                        app.set_activity(if amending {
                            "Amend complete."
                        } else {
                            "Commit complete."
                        });
                        app.selected_paths.clear();
                        app.selected_diff = None;
                        app.loaded_diff = None;
                        app.selected_diff_lines.clear();
                        app.file_list_select_all_toggle = None;
                        // Remember OID so History can select the new commit when opened later;
                        // stay on Working Copy (do not auto-navigate).
                        app.history_reveal_oid = Some(oid);
                        app.reset_history();
                        app.load_working_copy(repository, cx);
                    }
                    Err(error) => {
                        app.set_activity(git_failure_message(
                            if amending { "Amend" } else { "Commit" },
                            &error,
                        ));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn run_mutation(
        &mut self,
        operation: Mutation,
        paths: Vec<GitPath>,
        preserve_selection: bool,
        cx: &mut Context<Self>,
    ) {
        let ShellState::Repository(repository) = &self.state else {
            return;
        };
        let repository = repository.clone();
        let preserved_paths = if preserve_selection {
            self.selected_paths.clone()
        } else {
            Vec::new()
        };
        let preserved_diff = if preserve_selection {
            self.selected_diff.clone()
        } else {
            None
        };
        self.mutation_in_flight = true;
        self.set_activity(format!("{}…", operation.label()));
        let worker_repository = repository.clone();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    let git = GitExecutable::discover().map_err(|error| error.to_string())?;
                    match operation {
                        Mutation::StageSelected => git.stage_paths(&worker_repository, &paths),
                        Mutation::UnstageSelected => git.unstage_paths(&worker_repository, &paths),
                        Mutation::StageAll => git.stage_all(&worker_repository),
                        Mutation::UnstageAll => git.unstage_all(&worker_repository),
                        Mutation::DiscardSelected => {
                            discard_selected(&git, &worker_repository, &paths)
                        }
                    }
                    .map_err(|error| format!("{error:?}"))
                })
                .await;
            let _ = this.update(cx, |app, cx| {
                app.mutation_in_flight = false;
                match result {
                    Ok(()) => {
                        if preserve_selection {
                            app.selected_paths = preserved_paths;
                            app.selected_diff = preserved_diff;
                            app.loaded_diff = None;
                        } else {
                            app.selected_paths.clear();
                            app.loaded_diff = None;
                            app.selected_diff = None;
                            app.selected_diff_lines.clear();
                        }
                        app.set_activity(format!("{} complete.", operation.label()));
                        app.load_working_copy(repository.clone(), cx);
                        if preserve_selection
                            && let Some((path, staged)) = app.selected_diff.clone()
                        {
                            Self::load_diff(
                                repository,
                                path,
                                staged,
                                git_cli::MAX_DISPLAY_DIFF_BYTES,
                                cx,
                            );
                        }
                    }
                    Err(error) => app.set_activity(git_failure_message(operation.label(), &error)),
                }
                cx.notify();
            });
        })
        .detach();
    }
}

fn pull_command_args(remote_branch: &str, use_rebase: bool, remotes: &[String]) -> Vec<OsString> {
    let mut args = vec![OsString::from("pull"), OsString::from("--progress")];
    if use_rebase {
        args.push(OsString::from("--rebase"));
    }
    if remote_branch.is_empty() {
        return args;
    }
    if let Some((remote, branch)) = split_remote_tracking_ref(remote_branch, remotes) {
        args.push(OsString::from(remote));
        args.push(OsString::from(branch));
    } else {
        args.push(OsString::from(remote_branch));
    }
    args
}

/// Direction of a checkbox click in the Working Copy file list.
///
/// A click on a multi-file selection fills every checkbox in it, and only clears them once the
/// whole selection is staged — the clicked row's own state never decides for the group.
fn should_stage_selection(staged_flags: &[bool], clicked_currently_staged: bool) -> bool {
    if staged_flags.len() > 1 {
        !staged_flags.iter().all(|staged| *staged)
    } else {
        !clicked_currently_staged
    }
}

/// Push options composed by the Push dialog.
struct PushCommandOptions {
    head_branch: String,
    enabled: Vec<PushOption>,
    submodule_mode: SubmodulePushMode,
    set_upstream: bool,
}

impl PushCommandOptions {
    fn is_enabled(&self, option: PushOption) -> bool {
        self.enabled.contains(&option)
    }
}

fn push_command_args(
    destination: &str,
    options: &PushCommandOptions,
    remotes: &[String],
) -> Vec<OsString> {
    let mut args = vec![OsString::from("push"), OsString::from("--progress")];
    if options.is_enabled(PushOption::AllTags) {
        args.push(OsString::from("--tags"));
    }
    if options.is_enabled(PushOption::Force) {
        // Never a bare `--force`: a lease keeps unseen remote commits from being discarded.
        args.push(OsString::from("--force-with-lease"));
    }
    if options.is_enabled(PushOption::RecurseSubmodules) {
        args.push(OsString::from(format!(
            "--recurse-submodules={}",
            options.submodule_mode.flag_value()
        )));
    }
    if options.is_enabled(PushOption::SkipHooks) {
        args.push(OsString::from("--no-verify"));
    }
    if destination.is_empty() {
        return args;
    }
    if options.set_upstream {
        args.push(OsString::from("--set-upstream"));
    }
    if let Some((remote, branch)) = split_remote_tracking_ref(destination, remotes) {
        args.push(OsString::from(remote));
        args.push(OsString::from(format!("{}:{branch}", options.head_branch)));
    } else {
        args.push(OsString::from(destination));
    }
    args
}

fn split_remote_tracking_ref(remote_ref: &str, remotes: &[String]) -> Option<(String, String)> {
    for remote in remotes {
        let prefix = format!("{remote}/");
        if let Some(branch) = remote_ref.strip_prefix(&prefix)
            && !branch.is_empty()
        {
            return Some((remote.clone(), branch.to_owned()));
        }
    }
    let (remote, branch) = remote_ref.split_once('/')?;
    (!branch.is_empty()).then(|| (remote.to_owned(), branch.to_owned()))
}

#[cfg(test)]
mod pull_dialog_tests {
    use super::{pull_command_args, split_remote_tracking_ref};
    use std::ffi::OsString;

    #[test]
    fn splits_remote_tracking_refs_preferring_configured_remotes() {
        let remotes = vec!["origin".into()];
        assert_eq!(
            split_remote_tracking_ref("origin/feature/ui", &remotes),
            Some(("origin".into(), "feature/ui".into()))
        );
    }

    #[test]
    fn builds_rebase_pull_args_for_remote_branch() {
        let remotes = vec!["origin".into()];
        let args = pull_command_args("origin/main", true, &remotes);
        assert_eq!(
            args,
            vec![
                OsString::from("pull"),
                OsString::from("--progress"),
                OsString::from("--rebase"),
                OsString::from("origin"),
                OsString::from("main"),
            ]
        );
    }
}

#[cfg(test)]
mod staging_selection_tests {
    use super::should_stage_selection;

    #[test]
    fn mixed_selection_stages_every_file() {
        assert!(should_stage_selection(&[true, false, false], true));
        assert!(should_stage_selection(&[false, false], false));
    }

    #[test]
    fn fully_staged_selection_unstages_every_file() {
        assert!(!should_stage_selection(&[true, true, true], true));
    }

    #[test]
    fn single_file_click_toggles_that_file() {
        assert!(should_stage_selection(&[false], false));
        assert!(!should_stage_selection(&[true], true));
    }
}

#[cfg(test)]
mod push_dialog_tests {
    use super::{PushCommandOptions, PushOption, SubmodulePushMode, push_command_args};
    use std::ffi::OsString;

    fn options(head_branch: &str) -> PushCommandOptions {
        PushCommandOptions {
            head_branch: head_branch.to_owned(),
            enabled: Vec::new(),
            submodule_mode: SubmodulePushMode::Check,
            set_upstream: false,
        }
    }

    #[test]
    fn pushes_head_to_selected_remote_branch() {
        let remotes = vec!["origin".into()];
        let args = push_command_args("origin/main", &options("main"), &remotes);
        assert_eq!(
            args,
            vec![
                OsString::from("push"),
                OsString::from("--progress"),
                OsString::from("origin"),
                OsString::from("main:main"),
            ]
        );
    }

    #[test]
    fn force_push_uses_lease_and_carries_every_option() {
        let remotes = vec!["origin".into()];
        let mut options = options("feature/ui");
        options.enabled = vec![
            PushOption::AllTags,
            PushOption::Force,
            PushOption::RecurseSubmodules,
            PushOption::SkipHooks,
        ];
        options.submodule_mode = SubmodulePushMode::OnDemand;
        options.set_upstream = true;
        let args = push_command_args("origin/feature/ui", &options, &remotes);
        assert_eq!(
            args,
            vec![
                OsString::from("push"),
                OsString::from("--progress"),
                OsString::from("--tags"),
                OsString::from("--force-with-lease"),
                OsString::from("--recurse-submodules=on-demand"),
                OsString::from("--no-verify"),
                OsString::from("--set-upstream"),
                OsString::from("origin"),
                OsString::from("feature/ui:feature/ui"),
            ]
        );
    }

    #[test]
    fn empty_destination_falls_back_to_configured_upstream() {
        let args = push_command_args("", &options("main"), &["origin".into()]);
        assert_eq!(
            args,
            vec![OsString::from("push"), OsString::from("--progress")]
        );
    }
}
