//! macOS application entry point and state-mutating command methods.
//!
//! Window layout lives in `views/`; shared shell types live in `app_state`.
//! Keeping all state transitions here (never in render modules) preserves the
//! rule that Git and domain logic do not appear in GPUI render implementations.

mod actions;
mod app_state;
mod keymap;
mod menus;
#[cfg(test)]
mod tests;
mod views;

use std::{
    ffi::OsString,
    fs,
    os::unix::ffi::OsStringExt,
    path::{Path, PathBuf},
    process::Command,
    sync::{Arc, Mutex, mpsc},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use app_core::{
    RecentRepositoryStore, RecoveryJournalStore, RepositoryOpenError, WindowGeometry,
    open_repository,
};
use git_cli::{CommitRequest, GitExecutable, GitStatusError, read_stderr_limited};
use git_domain::{
    ConflictSide, FileHistoryRequest, GitPath, GraphState, HeadStatus, HistoryPage,
    HistoryReference, HistoryRequest, RefSnapshot, ReflogRequest, TreeEntry, TreeEntryKind,
    WorktreeRepository, layout_history_graph,
};
use gpui::{
    App, Application, Bounds, ClipboardItem, Context, ExternalPaths, ListAlignment, ListState,
    PathPromptOptions, Window, WindowBounds, WindowOptions, point, prelude::*, px, size,
};
use notify::{RecursiveMode, Watcher};
use ui_kit::Appearance;

use crate::actions::{
    CommandPalette, FocusComposer, HistoryNext, HistoryPrevious, NavigateBack, NavigateForward,
    OpenRepository, Refresh, ShortcutReference, ToggleAppearance, WidenInspector, WidenSidebar,
};
use crate::app_state::{
    ForcePushState, GitronimoApp, HistoryDetailMode, LastAction, Mutation, NetworkOperation,
    OpenedRepository, OperationAction, RefContext, RepositoryView, ShellState,
    ShortcutReferenceState, StashAction, ThemeMode, appearance_from_window, discard_selected,
    git_failure_message, network_failure_message, repository_is_available,
    repository_unavailable_message, resize_width,
};

const INITIAL_WINDOW_SIZE: (f32, f32) = (1200.0, 800.0);
const MINIMUM_WINDOW_SIZE: (f32, f32) = (800.0, 560.0);
fn main() {
    install_panic_reporter();
    Application::new().run(|cx: &mut App| {
        cx.bind_keys(keymap::bindings());
        cx.set_menus(menus::application_menus());

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
        let mut app = Self {
            focus_handle: cx.focus_handle(),
            last_action: None,
            appearance: appearance_from_window(window.appearance()),
            theme_mode: ThemeMode::System,
            sidebar_width: 220.0,
            inspector_width: 320.0,
            state: ShellState::Welcome,
            recents,
            activity: "Choose a repository to begin.".into(),
            working_copy: None,
            worktree_show_all_files: false,
            tracked_files: Vec::new(),
            refs: RefSnapshot::default(),
            expanded_ref_groups,
            ref_context: None,
            selected_paths: Vec::new(),
            context_path: None,
            loaded_diff: None,
            selected_diff: None,
            selected_diff_lines: Vec::new(),
            pending_line_discard: None,
            pending_hunk_discard: None,
            pending_discard: None,
            pending_stash_action: None,
            pending_operation_action: None,
            pending_branch_delete: None,
            force_push_state: ForcePushState::Idle,
            shortcut_reference_state: ShortcutReferenceState::Hidden,
            commit_subject: String::new(),
            commit_body: String::new(),
            commit_amend: false,
            commit_sign_off: false,
            author_identity: "Loading author identity…".into(),
            repository_view: RepositoryView::WorkingCopy,
            navigation_back: Vec::new(),
            navigation_forward: Vec::new(),
            history: Vec::new(),
            history_rows: Vec::new(),
            history_state: GraphState::default(),
            history_reference: HistoryReference::Current,
            history_next: None,
            history_decorations: Vec::new(),
            selected_history: None,
            history_search: String::new(),
            history_list_state: ListState::new(0, ListAlignment::Top, px(56.0)),
            history_paths: Vec::new(),
            history_diff: None,
            history_selection_token: 0,
            history_load_token: 0,
            history_reveal_oid: None,
            history_detail_mode: HistoryDetailMode::Changeset,
            stashes: Vec::new(),
            stashes_load_token: 0,
            selected_stash: None,
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
        };
        app.observe_system_appearance(window, cx);
        app.observe_window_geometry(window, cx);
        Self::load_diagnostics(cx);
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
        let mut app = Self {
            focus_handle: cx.focus_handle(),
            last_action: None,
            appearance: appearance_from_window(window.appearance()),
            theme_mode: ThemeMode::System,
            sidebar_width: 220.0,
            inspector_width: 320.0,
            state,
            recents,
            activity,
            working_copy: None,
            worktree_show_all_files: false,
            tracked_files: Vec::new(),
            refs: RefSnapshot::default(),
            expanded_ref_groups,
            ref_context: None,
            selected_paths: Vec::new(),
            context_path: None,
            loaded_diff: None,
            selected_diff: None,
            selected_diff_lines: Vec::new(),
            pending_line_discard: None,
            pending_hunk_discard: None,
            pending_discard: None,
            pending_stash_action: None,
            pending_operation_action: None,
            pending_branch_delete: None,
            force_push_state: ForcePushState::Idle,
            shortcut_reference_state: ShortcutReferenceState::Hidden,
            commit_subject: String::new(),
            commit_body: String::new(),
            commit_amend: false,
            commit_sign_off: false,
            author_identity: "Loading author identity…".into(),
            repository_view: RepositoryView::WorkingCopy,
            navigation_back: Vec::new(),
            navigation_forward: Vec::new(),
            history: Vec::new(),
            history_rows: Vec::new(),
            history_state: GraphState::default(),
            history_reference: HistoryReference::Current,
            history_next: None,
            history_decorations: Vec::new(),
            selected_history: None,
            history_search: String::new(),
            history_list_state: ListState::new(0, ListAlignment::Top, px(56.0)),
            history_paths: Vec::new(),
            history_diff: None,
            history_selection_token: 0,
            history_load_token: 0,
            history_reveal_oid: None,
            history_detail_mode: HistoryDetailMode::Changeset,
            stashes: Vec::new(),
            stashes_load_token: 0,
            selected_stash: None,
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
        };
        app.observe_system_appearance(window, cx);
        app.observe_window_geometry(window, cx);
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

    fn apply_open_outcome(
        &mut self,
        outcome: Result<OpenedRepository, RepositoryOpenError>,
        cx: &mut Context<Self>,
    ) {
        match outcome {
            Ok(opened) => {
                self.state = ShellState::Repository(opened.repository);
                self.recents = opened.recents;
                self.activity = "Repository opened.".into();
                self.working_copy = None;
                self.refs = RefSnapshot::default();
                self.ref_context = None;
                self.selected_paths.clear();
                self.context_path = None;
                self.loaded_diff = None;
                self.selected_diff = None;
                self.selected_diff_lines.clear();
                self.pending_line_discard = None;
                self.pending_hunk_discard = None;
                self.pending_discard = None;
                self.pending_stash_action = None;
                self.pending_operation_action = None;
                self.pending_branch_delete = None;
                self.force_push_state = ForcePushState::Idle;
                self.shortcut_reference_state = ShortcutReferenceState::Hidden;
                self.network_operation = None;
                self.commit_subject.clear();
                self.commit_body.clear();
                self.commit_amend = false;
                self.commit_sign_off = false;
                self.repository_view = RepositoryView::WorkingCopy;
                self.navigation_back.clear();
                self.navigation_forward.clear();
                self.history.clear();
                self.history_rows.clear();
                self.history_state = GraphState::default();
                self.history_reference = HistoryReference::Current;
                self.history_next = None;
                self.history_decorations.clear();
                self.selected_history = None;
                self.history_search.clear();
                self.history_list_state.reset(0);
                self.history_paths.clear();
                self.history_diff = None;
                self.history_selection_token = self.history_selection_token.wrapping_add(1);
                self.history_load_token = self.history_load_token.wrapping_add(1);
                self.history_detail_mode = HistoryDetailMode::Changeset;
                self.stashes.clear();
                self.stashes_load_token = self.stashes_load_token.wrapping_add(1);
                self.selected_stash = None;
                self.pending_stash_action_ref = None;
                self.lfs.clear();
                self.lfs_load_token = self.lfs_load_token.wrapping_add(1);
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
                self.activity = "Repository could not be opened.".into();
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
            self.activity = "Open a repository before refreshing its working copy.".into();
        }
        cx.notify();
    }

    fn focus_composer(&mut self, _: &FocusComposer, _: &mut Window, cx: &mut Context<Self>) {
        self.edit_commit_subject(cx);
    }

    fn show_command_palette(&mut self, _: &CommandPalette, _: &mut Window, cx: &mut Context<Self>) {
        self.activity = "Choose a command from the palette.".into();
        Self::prompt_command_palette(cx);
        cx.notify();
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

    fn prompt_command_palette(cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let command = cx
                .background_spawn(async {
                    Command::new("osascript")
                        .args(["-e", "choose from list {\"Refresh working copy\", \"Show history\", \"Commit detail…\", \"Show stashes\", \"Show remotes\", \"Git LFS status\", \"Show reflog\", \"File history…\", \"Blame…\", \"Compare refs…\", \"Browse tree at commit…\", \"Worktrees…\", \"Submodules…\", \"Rebase plan…\", \"Squash staged changes…\", \"Fixup staged changes…\", \"Drop commit…\", \"Reword last commit…\", \"Conflicts…\", \"Set merge tool…\", \"Open in merge tool…\", \"Check commit signature…\", \"Show working copy\", \"Show keyboard shortcuts\"} with title \"Gitronimo commands\" with prompt \"Choose an action\""])
                        .output()
                        .ok()
                        .filter(|output| output.status.success())
                        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
                        .filter(|command| command != "false")
                })
                .await;
            let _ = this.update(cx, |app, cx| match command.as_deref() {
                Some("Refresh working copy") => {
                    if let ShellState::Repository(repository) = &app.state {
                        app.load_working_copy(repository.clone(), cx);
                    } else {
                        app.activity = "Open a repository before refreshing its working copy.".into();
                    }
                    cx.notify();
                }
                Some("Show history") => {
                    if let ShellState::Repository(repository) = &app.state {
                        app.show_history(repository.clone(), cx);
                    }
                }
                Some("Commit detail…") => {
                    if let ShellState::Repository(repository) = &app.state {
                        if let Some(index) = app.selected_history {
                            let repository = repository.clone();
                            app.show_commit_detail(&repository, index, cx);
                        } else {
                            app.activity = "Select a history commit first.".into();
                        }
                    }
                }
                Some("Show stashes") => {
                    if let ShellState::Repository(repository) = &app.state {
                        app.show_stashes(repository.clone(), cx);
                    }
                }
                Some("Show remotes") => {
                    app.show_remotes(cx);
                }
                Some("Git LFS status") => {
                    if let ShellState::Repository(repository) = &app.state {
                        app.show_lfs(repository.clone(), cx);
                    }
                }
                Some("Show reflog") => {
                    if let ShellState::Repository(repository) = &app.state {
                        app.show_reflog(repository.clone(), cx);
                    }
                }
                Some("File history…") => GitronimoApp::prompt_file_history(cx),
                Some("Blame…") => GitronimoApp::prompt_blame(cx),
                Some("Compare refs…") => GitronimoApp::prompt_compare_refs(cx),
                Some("Browse tree at commit…") => GitronimoApp::prompt_browse_tree(cx),
                Some("Worktrees…") => {
                    if let ShellState::Repository(repository) = &app.state {
                        app.show_worktrees(repository.clone(), cx);
                    }
                }
                Some("Submodules…") => {
                    if let ShellState::Repository(repository) = &app.state {
                        app.show_submodules(repository.clone(), cx);
                    }
                }
                Some("Rebase plan…") => {
                    if let ShellState::Repository(repository) = &app.state {
                        app.show_rebase(repository.clone(), cx);
                    }
                }
                Some("Squash staged changes…") => GitronimoApp::prompt_autosquash(true, cx),
                Some("Fixup staged changes…") => GitronimoApp::prompt_autosquash(false, cx),
                Some("Drop commit…") => GitronimoApp::prompt_drop_commit(cx),
                Some("Reword last commit…") => GitronimoApp::prompt_reword_last_commit(cx),
                Some("Conflicts…") => {
                    if let ShellState::Repository(repository) = &app.state {
                        app.show_conflicts(repository.clone(), cx);
                    }
                }
                Some("Set merge tool…") => GitronimoApp::prompt_set_merge_tool(cx),
                Some("Open in merge tool…") => GitronimoApp::prompt_run_merge_tool(cx),
                Some("Check commit signature…") => GitronimoApp::prompt_check_commit_signature(cx),
                Some("Show working copy") => {
                    app.navigate_to(RepositoryView::WorkingCopy, cx);
                }
                Some("Show keyboard shortcuts") => {
                    app.shortcut_reference_state = ShortcutReferenceState::Visible;
                    cx.notify();
                }
                _ => {}
            });
        })
        .detach();
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

    fn navigate_to(&mut self, view: RepositoryView, cx: &mut Context<Self>) {
        if self.repository_view != view {
            self.navigation_back.push(self.repository_view);
            self.navigation_forward.clear();
            self.repository_view = view;
            cx.notify();
        }
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

    fn show_history(&mut self, repository: WorktreeRepository, cx: &mut Context<Self>) {
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
        let reference = self.history_reference.clone();
        let load_token = self.history_load_token;
        self.activity = "Loading history…".into();
        cx.spawn(async move |this, cx| {
            let result = cx.background_spawn(async move {
                let git = GitExecutable::discover().map_err(|error| error.to_string())?;
                let page = git.history_page(&repository, &HistoryRequest { reference, before, limit: 100 }).map_err(|error| format!("{error:?}"))?;
                let decorations = git.ref_decorations(&repository).map_err(|error| format!("{error:?}"))?;
                Ok::<_, String>((page, decorations))
            }).await;
            let _ = this.update(cx, |app, cx| {
                if !matches!(&app.state, ShellState::Repository(current) if current.worktree_root == root)
                    || app.history_load_token != load_token
                {
                    return;
                }
                match result {
                    Ok((HistoryPage { commits, next_before }, decorations)) => {
                        let rows = layout_history_graph(&commits, &mut app.history_state);
                        app.history.extend(commits); app.history_rows.extend(rows); app.history_next = next_before; app.history_decorations = decorations; app.activity = format!("Loaded {} history commits.", app.history.len());
                        app.history_list_state.reset(app.history_row_count());
                        if let Some(oid) = app.history_reveal_oid.take() {
                            app.selected_history =
                                app.history.iter().position(|commit| commit.oid == oid);
                        }
                    }
                    Err(error) => app.activity = format!("History load failed: {error}"),
                }
                cx.notify();
            });
        }).detach();
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
        self.activity = "Loading reflog…".into();
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
                        app.activity = format!("Loaded {} reflog entries.", app.reflog.len());
                    }
                    Err(error) => app.activity = format!("Reflog load failed: {error}"),
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
        self.activity = format!("Restoring branch {branch}…");
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
                        app.activity = format!("Restored branch {branch}.");
                        app.load_working_copy(repository.clone(), cx);
                        Self::load_refs(repository, cx);
                    }
                    Err(error) => {
                        app.activity =
                            git_failure_message(&format!("Restore branch {branch}"), &error);
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn prompt_file_history(cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let path = cx
                .background_spawn(async {
                    Command::new("osascript")
                        .args(["-e", "text returned of (display dialog \"File history for path\" default answer \"\")"])
                        .output()
                        .ok()
                        .filter(|output| output.status.success())
                        .map(|output| String::from_utf8_lossy(&output.stdout).trim_end().to_owned())
                        .filter(|path| !path.is_empty())
                })
                .await;
            let _ = this.update(cx, |app, cx| {
                if let Some(path) = path {
                    app.file_history_path = path;
                    let ShellState::Repository(repository) = &app.state else {
                        return;
                    };
                    let repository = repository.clone();
                    app.navigate_to(RepositoryView::FileHistory, cx);
                    app.load_file_history(repository, cx);
                    cx.notify();
                }
            });
        })
        .detach();
    }

    fn load_file_history(&mut self, repository: WorktreeRepository, cx: &mut Context<Self>) {
        let root = repository.worktree_root.clone();
        let load_token = self.file_history_load_token;
        let path = self.file_history_path.clone();
        self.activity = format!("Loading history for {path}…");
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
                        app.activity = format!(
                            "Loaded {} commits for {}.",
                            app.file_history.len(),
                            app.file_history_path
                        );
                    }
                    Err(error) => app.activity = format!("File history failed: {error}"),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn prompt_blame(cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let path = cx
                .background_spawn(async {
                    Command::new("osascript")
                        .args([
                            "-e",
                            "text returned of (display dialog \"Blame path\" default answer \"\")",
                        ])
                        .output()
                        .ok()
                        .filter(|output| output.status.success())
                        .map(|output| {
                            String::from_utf8_lossy(&output.stdout)
                                .trim_end()
                                .to_owned()
                        })
                        .filter(|path| !path.is_empty())
                })
                .await;
            let _ = this.update(cx, |app, cx| {
                if let Some(path) = path {
                    app.blame_path = path;
                    let ShellState::Repository(repository) = &app.state else {
                        return;
                    };
                    let repository = repository.clone();
                    app.navigate_to(RepositoryView::Blame, cx);
                    app.load_blame(repository, cx);
                    cx.notify();
                }
            });
        })
        .detach();
    }

    fn load_blame(&mut self, repository: WorktreeRepository, cx: &mut Context<Self>) {
        let root = repository.worktree_root.clone();
        let load_token = self.blame_load_token;
        let path = self.blame_path.clone();
        self.activity = format!("Loading blame for {path}…");
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
                        app.activity = format!(
                            "Loaded blame for {} ({} lines).",
                            app.blame_path,
                            app.blame.len()
                        );
                    }
                    Err(error) => app.activity = format!("Blame failed: {error}"),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn prompt_compare_refs(cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let left = cx
                .background_spawn(async {
                    Command::new("osascript")
                        .args(["-e", "text returned of (display dialog \"Compare from ref\" default answer \"HEAD\")"])
                        .output()
                        .ok()
                        .filter(|output| output.status.success())
                        .map(|output| String::from_utf8_lossy(&output.stdout).trim_end().to_owned())
                        .filter(|name| !name.is_empty())
                })
                .await;
            let right = cx
                .background_spawn(async {
                    Command::new("osascript")
                        .args(["-e", "text returned of (display dialog \"Compare to ref\" default answer \"\")"])
                        .output()
                        .ok()
                        .filter(|output| output.status.success())
                        .map(|output| String::from_utf8_lossy(&output.stdout).trim_end().to_owned())
                        .filter(|name| !name.is_empty())
                })
                .await;
            let _ = this.update(cx, |app, cx| {
                let (Some(left), Some(right)) = (left, right) else {
                    return;
                };
                app.compare_left = left;
                app.compare_right = right;
                let ShellState::Repository(repository) = &app.state else {
                    return;
                };
                let repository = repository.clone();
                app.navigate_to(RepositoryView::Compare, cx);
                app.load_compare(repository, cx);
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
        self.activity = format!("Comparing {left}…{right}…");
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
                        app.activity = format!(
                            "Compared {}…{} ({} file(s)).",
                            app.compare_left,
                            app.compare_right,
                            app.compare_diff
                                .as_ref()
                                .map_or(0, |loaded| loaded.diff.files.len())
                        );
                    }
                    Err(error) => app.activity = format!("Compare failed: {error}"),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn prompt_browse_tree(cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let oid = cx
                .background_spawn(async {
                    Command::new("osascript")
                        .args(["-e", "text returned of (display dialog \"Browse tree at commit\" default answer \"HEAD\")"])
                        .output()
                        .ok()
                        .filter(|output| output.status.success())
                        .map(|output| String::from_utf8_lossy(&output.stdout).trim_end().to_owned())
                        .filter(|name| !name.is_empty())
                })
                .await;
            let _ = this.update(cx, |app, cx| {
                if let Some(oid) = oid {
                    app.tree_oid = oid;
                    let ShellState::Repository(repository) = &app.state else {
                        return;
                    };
                    let repository = repository.clone();
                    app.navigate_to(RepositoryView::Tree, cx);
                    app.tree_path.clear();
                    app.tree_blob = None;
                    app.load_tree(repository, cx);
                    cx.notify();
                }
            });
        })
        .detach();
    }

    fn load_tree(&mut self, repository: WorktreeRepository, cx: &mut Context<Self>) {
        let root = repository.worktree_root.clone();
        let load_token = self.tree_load_token;
        let oid = self.tree_oid.clone();
        let path = joined_tree_path(&self.tree_path);
        self.activity = format!("Loading tree {}…", self.tree_path_label());
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
                        app.activity = format!("Listed {} entries.", app.tree.len());
                    }
                    Err(error) => app.activity = format!("Tree load failed: {error}"),
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
                self.activity = format!("Reading {}", String::from_utf8_lossy(&full_path.0));
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
                                app.activity = String::new();
                            }
                            Err(error) => app.activity = format!("Blob read failed: {error}"),
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
                    app.activity = "Select a file in the tree before exporting.".into();
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
                    app.activity = "The selected blob has no file name.".into();
                    cx.notify();
                    return;
                };
                let path = std::path::Path::new(&destination).join(file_name);
                match std::fs::write(&path, blob) {
                    Ok(()) => {
                        app.activity = format!("Exported to {}.", path.display());
                    }
                    Err(error) => {
                        app.activity = format!("Export failed: {error}");
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
        self.activity = "Loading worktrees…".into();
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
                        app.activity = format!("Loaded {} worktree(s).", app.worktrees.len());
                    }
                    Err(error) => app.activity = format!("Worktree load failed: {error}"),
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
        self.activity = format!("{label}…");
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
                        app.activity = format!("{label} complete.");
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
                    Err(error) => app.activity = git_failure_message(&label, &error),
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
        self.activity = "Loading submodules…".into();
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
                        app.activity = format!("Loaded {} submodule(s).", app.submodules.len());
                    }
                    Err(error) => app.activity = format!("Submodule load failed: {error}"),
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
        self.activity = "Loading Git LFS status…".into();
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
                        app.activity = format!("Loaded {} Git LFS change(s).", app.lfs.len());
                    }
                    Err(error) => app.activity = git_failure_message("Git LFS status", &error),
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
        self.activity = "Loading rebase plan…".into();
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
                        app.activity = format!("Loaded {} rebase step(s).", app.rebase_plan.len());
                    }
                    Err(error) => {
                        app.rebase_plan = Vec::new();
                        app.activity = format!("Rebase plan load failed: {error}");
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

    fn prompt_start_rebase(cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let base = cx
                .background_spawn(async {
                    Command::new("osascript")
                        .args(["-e", "text returned of (display dialog \"Rebase current branch onto\" default answer \"main\")"])
                        .output()
                        .ok()
                        .filter(|output| output.status.success())
                        .map(|output| String::from_utf8_lossy(&output.stdout).trim_end().to_owned())
                        .filter(|base| !base.is_empty())
                })
                .await;
            let _ = this.update(cx, |app, cx| {
                if let Some(base) = base {
                    app.run_worktree_mutation(
                        format!("Rebase onto {base}"),
                        move |git, repository| git.start_rebase(repository, &base),
                        cx,
                    );
                }
            });
        })
        .detach();
    }

    fn prompt_autosquash(squash: bool, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let target = cx
                .background_spawn(async {
                    Command::new("osascript")
                        .args(["-e", "text returned of (display dialog \"Fold staged changes into commit\" default answer \"HEAD\")"])
                        .output()
                        .ok()
                        .filter(|output| output.status.success())
                        .map(|output| String::from_utf8_lossy(&output.stdout).trim_end().to_owned())
                        .filter(|target| !target.is_empty())
                })
                .await;
            let message = if squash {
                cx.background_spawn(async {
                    Command::new("osascript")
                        .args(["-e", "text returned of (display dialog \"Squash message\" default answer \"\")"])
                        .output()
                        .ok()
                        .filter(|output| output.status.success())
                        .map(|output| String::from_utf8_lossy(&output.stdout).trim_end().to_owned())
                        .filter(|message| !message.is_empty())
                })
                .await
            } else {
                None
            };
            let _ = this.update(cx, |app, cx| {
                let Some(target) = target else {
                    return;
                };
                let label = if squash { "Squash into" } else { "Fixup into" };
                app.run_worktree_mutation(
                    format!("{label} {target}"),
                    move |git, repository| git.autosquash(repository, &target, message.as_deref()),
                    cx,
                );
            });
        })
        .detach();
    }

    fn prompt_drop_commit(cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let target = cx
                .background_spawn(async {
                    Command::new("osascript")
                        .args(["-e", "text returned of (display dialog \"Drop commit from history (e.g. HEAD~2 or an oid)\" default answer \"HEAD~1\")"])
                        .output()
                        .ok()
                        .filter(|output| output.status.success())
                        .map(|output| String::from_utf8_lossy(&output.stdout).trim_end().to_owned())
                        .filter(|target| !target.is_empty())
                })
                .await;
            let _ = this.update(cx, |app, cx| {
                if let Some(target) = target {
                    app.run_worktree_mutation(
                        format!("Drop {target}"),
                        move |git, repository| git.drop_commit(repository, &target),
                        cx,
                    );
                }
            });
        })
        .detach();
    }

    fn prompt_reword_last_commit(cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let subject = cx
                .background_spawn(async {
                    Command::new("osascript")
                        .args(["-e", "text returned of (display dialog \"New commit subject\" default answer \"\")"])
                        .output()
                        .ok()
                        .filter(|output| output.status.success())
                        .map(|output| String::from_utf8_lossy(&output.stdout).trim_end().to_owned())
                        .filter(|subject| !subject.is_empty())
                })
                .await;
            let body = cx
                .background_spawn(async {
                    Command::new("osascript")
                        .args(["-e", "text returned of (display dialog \"New commit body (optional)\" default answer \"\")"])
                        .output()
                        .ok()
                        .filter(|output| output.status.success())
                        .map(|output| String::from_utf8_lossy(&output.stdout).trim_end().to_owned())
                })
                .await;
            let _ = this.update(cx, |app, cx| {
                let Some(subject) = subject else {
                    return;
                };
                let body = body.unwrap_or_default();
                app.run_worktree_mutation(
                    "Reword last commit".to_owned(),
                    move |git, repository| {
                        git.commit(
                            repository,
                            &CommitRequest {
                                subject,
                                body,
                                amend: true,
                                sign_off: false,
                            },
                        )
                    },
                    cx,
                );
            });
        })
        .detach();
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
                        app.activity = format!("Conflict read failed: {error}");
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

    fn prompt_set_merge_tool(cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let tool = cx
                .background_spawn(async {
                    Command::new("osascript")
                        .args(["-e", "choose from list {\"opendiff\", \"meld\", \"kdiff3\", \"vimdiff\", \"bc3\"} with title \"Gitronimo merge tool\" with prompt \"Choose a merge tool\""])
                        .output()
                        .ok()
                        .filter(|output| output.status.success())
                        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
                        .filter(|tool| tool != "false" && !tool.is_empty())
                })
                .await;
            let _ = this.update(cx, |app, cx| {
                if let Some(tool) = tool {
                    app.run_worktree_mutation(
                        format!("Set merge tool to {tool}"),
                        move |git, repository| git.set_merge_tool(repository, &tool),
                        cx,
                    );
                }
            });
        })
        .detach();
    }

    fn prompt_run_merge_tool(cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let path = cx
                .background_spawn(async {
                    Command::new("osascript")
                        .args(["-e", "text returned of (display dialog \"Conflicted path, or leave empty for all\" default answer \"\")"])
                        .output()
                        .ok()
                        .filter(|output| output.status.success())
                        .map(|output| String::from_utf8_lossy(&output.stdout).trim_end().to_owned())
                        .filter(|path| !path.is_empty())
                })
                .await;
            let _ = this.update(cx, |app, cx| {
                let ShellState::Repository(repository) = &app.state else {
                    app.activity = "Open a repository before using the merge tool.".into();
                    return;
                };
                let repository = repository.clone();
                let worker_repository = repository.clone();
                let path_arg = path.map(|path| GitPath(path.as_bytes().to_vec()));
                app.activity = "Opening merge tool…".into();
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
                            app.activity = "Merge tool finished.".into();
                            app.load_working_copy(repository, cx);
                        }
                        Err(error) => app.activity = format!("Merge tool failed: {error}"),
                    });
                })
                .detach();
            });
        })
        .detach();
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
                    app.activity = "Open a repository before verifying a commit.".into();
                    return;
                };
                let repository = repository.clone();
                let worker_repository = repository.clone();
                let worker_oid = oid.clone();
                app.activity = format!("Checking signature of {oid}…");
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
                                app.activity = if signature.signer.is_empty() {
                                    format!("Commit {oid} signature: {label}.")
                                } else {
                                    format!(
                                        "Commit {oid} signature: {label} (signed by {}).",
                                        signature.signer
                                    )
                                };
                            }
                            Err(error) => {
                                app.activity =
                                    git_failure_message(&format!("Verify commit {oid}"), &error);
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
        self.appearance = match self.theme_mode {
            ThemeMode::System => appearance_from_window(window.appearance()),
            ThemeMode::Light => Appearance::Light,
            ThemeMode::Dark => Appearance::Dark,
        };
        cx.notify();
    }

    fn widen_sidebar(&mut self, _: &WidenSidebar, _: &mut Window, cx: &mut Context<Self>) {
        self.sidebar_width = resize_width(self.sidebar_width);
        cx.notify();
    }

    fn widen_inspector(&mut self, _: &WidenInspector, _: &mut Window, cx: &mut Context<Self>) {
        self.inspector_width = resize_width(self.inspector_width);
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
        self.activity =
            "Discard the unsaved commit draft before opening another repository?".into();
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
                    app.begin_open_path(path, window, cx);
                } else {
                    app.activity = "Kept the unsaved commit draft.".into();
                    cx.notify();
                }
            });
        })
        .detach();
    }

    fn begin_open_path(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        self.state = ShellState::Loading(path.clone());
        self.stop_watcher();
        self.activity = "Opening repository…".into();
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
            self.activity =
                "Repository is no longer available. Choose it again when it returns.".into();
            cx.notify();
            return;
        }
        let root = repository.worktree_root.clone();
        self.activity = "Refreshing working copy…".into();
        cx.spawn(async move |this, cx| {
            let status = cx
                .background_spawn(async move {
                    GitExecutable::discover()
                        .map_err(|error| error.to_string())?
                        .worktree_status(&repository, false)
                        .map_err(|error| format!("{error:?}"))
                })
                .await;
            let _ = this.update(cx, |app, cx| {
                let ShellState::Repository(current) = &app.state else {
                    return;
                };
                if current.worktree_root != root {
                    return;
                }
                match status {
                    Ok(status) => {
                        app.activity = format!(
                            "Working copy refreshed: {} change(s).",
                            status.entries.len()
                        );
                        app.working_copy = Some(status);
                    }
                    Err(error) => app.activity = format!("Working copy refresh failed: {error}"),
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
                        Err(error) => app.activity = format!("Ref load failed: {error}"),
                    }
                    cx.notify();
                }
            });
        })
        .detach();
    }

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

    fn create_branch(&mut self, branch: String, cx: &mut Context<Self>) {
        self.create_branch_from(branch, None, cx);
    }

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
                    app.activity = "Checkout a local branch before renaming it.".into();
                    cx.notify();
                    return;
                };
                app.run_branch_command(format!("Renaming {current} to {name}"), move |git, repository| {
                    git.rename_branch(repository, &current, &name)
                }, cx);
            });
        }).detach();
    }

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
                    app.activity = format!("Unknown local branch: {branch}");
                    cx.notify();
                    return;
                }
                app.pending_branch_delete = Some(branch.clone());
                app.activity = format!("Review deletion choices for local branch {branch}.");
                cx.notify();
            });
        }).detach();
    }

    fn confirm_branch_delete(&mut self, force: bool, cx: &mut Context<Self>) {
        let Some(branch) = self.pending_branch_delete.clone() else {
            return;
        };
        let label = if force {
            format!("Force deleting unmerged branch {branch}")
        } else {
            format!("Deleting merged branch {branch}")
        };
        self.run_branch_command(
            label,
            move |git, repository| git.delete_branch(repository, &branch, force),
            cx,
        );
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

    fn prompt_branch_from_selected(cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let name = cx.background_spawn(async {
                Command::new("osascript")
                    .args(["-e", "text returned of (display dialog \"New branch from selected commit\" default answer \"\")"])
                    .output().ok().filter(|output| output.status.success())
                    .map(|output| String::from_utf8_lossy(&output.stdout).trim_end().to_owned())
                    .filter(|name| !name.is_empty())
            }).await;
            let _ = this.update(cx, |app, cx| {
                let Some(name) = name else { return; };
                let Some(oid) = app.selected_history.and_then(|index| app.history.get(index)).map(|commit| commit.oid.clone()) else {
                    app.activity = "Select a history commit first.".into();
                    cx.notify();
                    return;
                };
                app.create_branch_from(name, Some(oid), cx);
            });
        }).detach();
    }

    fn default_remote(&self) -> Option<String> {
        self.refs
            .remotes
            .first()
            .and_then(|remote| String::from_utf8(remote.name.0.clone()).ok())
    }

    fn has_upstream(&self) -> bool {
        self.working_copy
            .as_ref()
            .is_some_and(|status| status.branch.upstream.is_some())
    }

    fn has_attached_branch(&self) -> bool {
        self.working_copy
            .as_ref()
            .is_some_and(|status| matches!(status.branch.head, HeadStatus::Branch(_)))
    }

    fn fetch_default_remote(&mut self, cx: &mut Context<Self>) {
        let Some(remote) = self.default_remote() else {
            self.activity = "No configured remote to fetch.".into();
            cx.notify();
            return;
        };
        self.run_network_command(
            format!("Fetching {remote}"),
            vec!["fetch".into(), "--progress".into(), remote.into()],
            cx,
        );
    }

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
                    app.activity = format!("Unknown configured remote: {remote}");
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

    fn pull_current(&mut self, cx: &mut Context<Self>) {
        self.run_network_command(
            "Pulling current branch".into(),
            vec!["pull".into(), "--progress".into()],
            cx,
        );
    }

    fn push_current(&mut self, cx: &mut Context<Self>) {
        self.run_network_command(
            "Pushing current branch".into(),
            vec!["push".into(), "--progress".into()],
            cx,
        );
    }

    fn publish_current(&mut self, cx: &mut Context<Self>) {
        let Some(remote) = self.default_remote() else {
            self.activity = "No configured remote to publish to.".into();
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
            self.activity = "Checkout a local branch before publishing.".into();
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

    fn request_force_with_lease(&mut self, cx: &mut Context<Self>) {
        self.force_push_state = ForcePushState::AwaitingConfirmation;
        self.activity =
            "Force-with-lease can rewrite the remote branch. Review and confirm.".into();
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
        self.activity = format!("{label} in progress. You can cancel it.");
        cx.spawn(async move |this, cx| {
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
                    let progress =
                        read_stderr_limited(stderr).map_err(|error| error.to_string())?;
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
                match result {
                    Ok(()) => {
                        app.activity = format!("{label} complete.");
                        app.load_working_copy(repository.clone(), cx);
                        Self::load_refs(repository, cx);
                    }
                    Err(error) if error == "cancelled" => {
                        app.activity = format!("{label} cancelled.");
                    }
                    Err(error) => app.activity = network_failure_message(&label, &error),
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
        self.activity = if cancelled {
            "Cancelling network operation…".into()
        } else {
            "Unable to cancel the network operation.".into()
        };
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
        self.activity = format!("{label}…");
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
                        app.activity = format!("{label} complete.");
                        app.load_working_copy(repository.clone(), cx);
                        Self::load_refs(repository, cx);
                    }
                    Err(error) => app.activity = git_failure_message(&label, &error),
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
            self.activity = "Ref group expansion could not be saved.".into();
        }
        cx.notify();
    }

    fn select_ref_context(&mut self, context: RefContext, cx: &mut Context<Self>) {
        self.ref_context = Some(context);
        cx.notify();
    }

    fn prompt_branch_from_ref(start: String, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let name = cx
                .background_spawn(async {
                    Command::new("osascript")
                        .args(["-e", "text returned of (display dialog \"New branch from ref\" default answer \"\")"])
                        .output()
                        .ok()
                        .filter(|output| output.status.success())
                        .map(|output| String::from_utf8_lossy(&output.stdout).trim_end().to_owned())
                        .filter(|name| !name.is_empty())
                })
                .await;
            let _ = this.update(cx, |app, cx| {
                if let Some(name) = name {
                    app.create_branch_from(name, Some(start), cx);
                }
            });
        })
        .detach();
    }

    fn show_ref_history(&mut self, reference: String, cx: &mut Context<Self>) {
        let ShellState::Repository(repository) = &self.state else {
            return;
        };
        let repository = repository.clone();
        self.navigate_to(RepositoryView::History, cx);
        self.change_history_reference(HistoryReference::Named(reference), repository, cx);
    }

    fn prompt_history_search(cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let search = cx.background_spawn(async {
                Command::new("osascript")
                    .args(["-e", "text returned of (display dialog \"Search loaded history\" default answer \"\")"])
                    .output().ok().filter(|output| output.status.success())
                    .map(|output| String::from_utf8_lossy(&output.stdout).trim_end().to_owned())
            }).await;
            let _ = this.update(cx, |app, cx| {
                if let Some(search) = search {
                    app.history_search = search;
                    app.history_list_state.reset(app.history_row_count());
                    cx.notify();
                }
            });
        }).detach();
    }

    fn prompt_history_reference(cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let reference = cx.background_spawn(async {
                Command::new("osascript")
                    .args(["-e", "text returned of (display dialog \"Branch or tag history\" default answer \"\")"])
                    .output().ok().filter(|output| output.status.success())
                    .map(|output| String::from_utf8_lossy(&output.stdout).trim_end().to_owned())
                    .filter(|reference| !reference.is_empty())
            }).await;
            let _ = this.update(cx, |app, cx| {
                let Some(reference) = reference else { return; };
                let ShellState::Repository(repository) = &app.state else { return; };
                app.change_history_reference(HistoryReference::Named(reference), repository.clone(), cx);
            });
        }).detach();
    }

    fn copy_selected_history_oid(&mut self, cx: &mut Context<Self>) {
        if let Some(commit) = self
            .selected_history
            .and_then(|index| self.history.get(index))
        {
            cx.write_to_clipboard(ClipboardItem::new_string(commit.oid.clone()));
            self.activity = "Commit OID copied.".into();
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
                        app.activity =
                            "Repository is no longer available. Choose it again when it returns."
                                .into();
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
        staged: bool,
        cx: &mut Context<Self>,
    ) {
        if additive {
            if let Some(index) = self
                .selected_paths
                .iter()
                .position(|selected| selected == &path)
            {
                self.selected_paths.remove(index);
            } else {
                self.selected_paths.push(path);
            }
        } else {
            self.selected_paths = vec![path];
        }
        if !additive && let ShellState::Repository(repository) = &self.state {
            self.selected_diff = Some((self.selected_paths[0].clone(), staged));
            Self::load_diff(
                repository.clone(),
                self.selected_paths[0].clone(),
                staged,
                git_cli::MAX_DISPLAY_DIFF_BYTES,
                cx,
            );
        }
        cx.notify();
    }

    fn load_diff(
        repository: WorktreeRepository,
        path: GitPath,
        staged: bool,
        limit: usize,
        cx: &mut Context<Self>,
    ) {
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
        path: GitPath,
        currently_staged: bool,
        cx: &mut Context<Self>,
    ) {
        if self.mutation_in_flight {
            return;
        }
        self.selected_paths = vec![path];
        let operation = if currently_staged {
            Mutation::UnstageSelected
        } else {
            Mutation::StageSelected
        };
        self.mutate(operation, cx);
    }

    fn toggle_worktree_show_all(&mut self, cx: &mut Context<Self>) {
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
                                app.activity = git_failure_message("List all files", &error);
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
        self.activity = "Loading full diff…".into();
        Self::load_diff(repository.clone(), path, staged, usize::MAX, cx);
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
        self.activity = "Staging hunk…".into();
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
                        app.activity = "Hunk staged.".into();
                        app.load_working_copy(repository.clone(), cx);
                        Self::load_diff(
                            repository,
                            path,
                            false,
                            git_cli::MAX_DISPLAY_DIFF_BYTES,
                            cx,
                        );
                    }
                    Err(error) => app.activity = git_failure_message("Stage hunk", &error),
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
        self.activity = "Unstaging hunk…".into();
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
                        app.activity = "Hunk unstaged.".into();
                        app.load_working_copy(repository.clone(), cx);
                        Self::load_diff(
                            repository,
                            path,
                            true,
                            git_cli::MAX_DISPLAY_DIFF_BYTES,
                            cx,
                        );
                    }
                    Err(error) => app.activity = git_failure_message("Unstage hunk", &error),
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
        self.activity = "Path copied.".into();
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
        self.activity = if reveal {
            "Revealing file in Finder…"
        } else {
            "Opening file…"
        }
        .into();
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
            self.activity = "Select at least one file first.".into();
            cx.notify();
            return;
        }
        if operation == Mutation::DiscardSelected && self.pending_discard.as_ref() != Some(&paths) {
            self.pending_discard = Some(paths);
            self.activity = "Review the discard consequences, then confirm.".into();
            cx.notify();
            return;
        }
        self.run_mutation(operation, paths, cx);
    }

    fn confirm_discard(&mut self, cx: &mut Context<Self>) {
        let Some(paths) = self.pending_discard.take() else {
            return;
        };
        self.run_mutation(Mutation::DiscardSelected, paths, cx);
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
        self.activity = "Staging selected lines…".into();
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
                        app.activity = "Selected lines staged.".into();
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
                        app.activity = git_failure_message("Stage selected lines", &error);
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
        self.activity = "Review the line discard consequences, then confirm.".into();
        cx.notify();
    }

    fn cancel_line_discard(&mut self, cx: &mut Context<Self>) {
        self.pending_line_discard = None;
        self.activity = "Line discard cancelled.".into();
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
        self.activity = "Discarding selected lines…".into();
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
                        app.activity = "Selected lines discarded.".into();
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
                        app.activity = git_failure_message("Discard selected lines", &error);
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
        self.activity = "Review the hunk discard consequences, then confirm.".into();
        cx.notify();
    }

    fn cancel_hunk_discard(&mut self, cx: &mut Context<Self>) {
        self.pending_hunk_discard = None;
        self.activity = "Hunk discard cancelled.".into();
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
        self.activity = "Discarding hunk…".into();
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
                        app.activity = "Hunk discarded.".into();
                        app.load_working_copy(repository.clone(), cx);
                        Self::load_diff(
                            repository,
                            path,
                            false,
                            git_cli::MAX_DISPLAY_DIFF_BYTES,
                            cx,
                        );
                    }
                    Err(error) => app.activity = git_failure_message("Discard hunk", &error),
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
        self.activity =
            "Aborting discards the paused operation and returns to its start state.".into();
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
        self.activity =
            "Resolve conflicts, stage the resolved files, then confirm to continue the operation."
                .into();
        cx.notify();
    }

    fn cancel_operation_action(&mut self, cx: &mut Context<Self>) {
        self.pending_operation_action = None;
        self.activity = "Operation action cancelled.".into();
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
        self.activity = match action {
            OperationAction::Abort => "Aborting operation…".into(),
            OperationAction::Continue => "Continuing operation…".into(),
        };
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
                        app.activity = match journal_warning {
                            Some(warning) => format!("{base} {warning}"),
                            None => base.into(),
                        };
                        app.load_working_copy(repository, cx);
                    }
                    Err(error) => {
                        let label = match action {
                            OperationAction::Abort => "Abort operation",
                            OperationAction::Continue => "Continue operation",
                        };
                        app.activity = git_failure_message(label, &error);
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
        self.activity = "Loading stashes…".into();
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
                        app.activity = format!("Loaded {} stash entr(ies).", app.stashes.len());
                    }
                    Err(error) => app.activity = git_failure_message("List stashes", &error),
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
        self.selected_stash = Some(index);
        cx.notify();
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

    fn apply_stash_by_selection(&mut self, cx: &mut Context<Self>) {
        if self.mutation_in_flight {
            return;
        }
        let Some((reference, _)) = self.selected_stash() else {
            self.activity = "Select a stash first.".into();
            cx.notify();
            return;
        };
        let ShellState::Repository(repository) = &self.state else {
            return;
        };
        let repository = repository.clone();
        let worker_repository = repository.clone();
        let worker_reference = reference.clone();
        self.mutation_in_flight = true;
        self.activity = format!("Applying {reference}…");
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    GitExecutable::discover()
                        .map_err(|error| error.to_string())?
                        .apply_stash(&worker_repository, &worker_reference)
                        .map_err(|error| format!("{error:?}"))
                })
                .await;
            let _ = this.update(cx, |app, cx| {
                app.mutation_in_flight = false;
                match result {
                    Ok(()) => {
                        app.activity = format!("{reference} applied; its recovery entry remains.");
                        app.load_working_copy(repository.clone(), cx);
                        app.load_stashes(repository, cx);
                    }
                    Err(error) => app.activity = git_failure_message("Apply stash", &error),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn request_stash_action(&mut self, action: StashAction, cx: &mut Context<Self>) {
        let Some((reference, subject)) = self.selected_stash() else {
            self.activity = "Select a stash first.".into();
            cx.notify();
            return;
        };
        self.pending_stash_action_ref = Some((action, reference, subject));
        self.activity = match action {
            StashAction::Pop => "Confirm before removing the stash recovery entry.".into(),
            StashAction::Drop => "Confirm before permanently removing the stash.".into(),
        };
        cx.notify();
    }

    fn cancel_stash_action_ref(&mut self, cx: &mut Context<Self>) {
        self.pending_stash_action_ref = None;
        cx.notify();
    }

    fn confirm_stash_action_ref(&mut self, cx: &mut Context<Self>) {
        let Some((action, reference, _)) = self.pending_stash_action_ref.take() else {
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
        let worker_reference = reference.clone();
        self.mutation_in_flight = true;
        let action_label = match action {
            StashAction::Pop => "Pop stash",
            StashAction::Drop => "Drop stash",
        };
        self.activity = format!("{action_label} {reference}…");
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    let git = GitExecutable::discover().map_err(|error| error.to_string())?;
                    let outcome = match action {
                        StashAction::Pop => git.pop_stash(&worker_repository, &worker_reference),
                        StashAction::Drop => git.drop_stash(&worker_repository, &worker_reference),
                    };
                    outcome.map_err(|error| format!("{error:?}"))
                })
                .await;
            let _ = this.update(cx, |app, cx| {
                app.mutation_in_flight = false;
                match result {
                    Ok(()) => {
                        app.activity = format!("{action_label} {reference} complete.");
                        app.load_working_copy(repository.clone(), cx);
                        app.load_stashes(repository, cx);
                    }
                    Err(error) => app.activity = git_failure_message(action_label, &error),
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

    fn create_stash(&mut self, include_untracked: bool, cx: &mut Context<Self>) {
        if self.mutation_in_flight {
            return;
        }
        let ShellState::Repository(repository) = &self.state else {
            return;
        };
        let repository = repository.clone();
        let worker_repository = repository.clone();
        self.mutation_in_flight = true;
        self.activity = "Creating stash…".into();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    GitExecutable::discover()
                        .map_err(|error| error.to_string())?
                        .create_stash(&worker_repository, include_untracked)
                        .map_err(|error| format!("{error:?}"))
                })
                .await;
            let _ = this.update(cx, |app, cx| {
                app.mutation_in_flight = false;
                match result {
                    Ok(()) => {
                        app.activity = "Stash created.".into();
                        app.load_working_copy(repository, cx);
                    }
                    Err(error) => app.activity = git_failure_message("Create stash", &error),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn apply_latest_stash(&mut self, cx: &mut Context<Self>) {
        if self.mutation_in_flight {
            return;
        }
        let ShellState::Repository(repository) = &self.state else {
            return;
        };
        let repository = repository.clone();
        let worker_repository = repository.clone();
        self.mutation_in_flight = true;
        self.activity = "Applying latest stash…".into();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    GitExecutable::discover()
                        .map_err(|error| error.to_string())?
                        .apply_latest_stash(&worker_repository)
                        .map_err(|error| format!("{error:?}"))
                })
                .await;
            let _ = this.update(cx, |app, cx| {
                app.mutation_in_flight = false;
                match result {
                    Ok(()) => {
                        app.activity =
                            "Latest stash applied; it remains available for recovery.".into();
                        app.load_working_copy(repository, cx);
                    }
                    Err(error) => app.activity = git_failure_message("Apply latest stash", &error),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn pop_latest_stash(&mut self, cx: &mut Context<Self>) {
        let ShellState::Repository(repository) = &self.state else {
            return;
        };
        let repository = repository.clone();
        let worker_repository = repository.clone();
        self.pending_stash_action = None;
        self.mutation_in_flight = true;
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    GitExecutable::discover()
                        .map_err(|error| error.to_string())?
                        .pop_latest_stash(&worker_repository)
                        .map_err(|error| format!("{error:?}"))
                })
                .await;
            let _ = this.update(cx, |app, cx| {
                app.mutation_in_flight = false;
                match result {
                    Ok(()) => {
                        app.activity = "Latest stash applied and removed.".into();
                        app.load_working_copy(repository, cx);
                    }
                    Err(error) => app.activity = git_failure_message("Pop latest stash", &error),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn drop_latest_stash(&mut self, cx: &mut Context<Self>) {
        let ShellState::Repository(repository) = &self.state else {
            return;
        };
        let repository = repository.clone();
        let worker_repository = repository.clone();
        self.pending_stash_action = None;
        self.mutation_in_flight = true;
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    GitExecutable::discover()
                        .map_err(|error| error.to_string())?
                        .drop_latest_stash(&worker_repository)
                        .map_err(|error| format!("{error:?}"))
                })
                .await;
            let _ = this.update(cx, |app, cx| {
                app.mutation_in_flight = false;
                match result {
                    Ok(()) => {
                        app.activity = "Latest stash removed.".into();
                        app.load_working_copy(repository, cx);
                    }
                    Err(error) => app.activity = git_failure_message("Drop latest stash", &error),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn edit_commit_subject(&mut self, cx: &mut Context<Self>) {
        self.prompt_commit_text(true, cx);
    }
    fn edit_commit_body(&mut self, cx: &mut Context<Self>) {
        self.prompt_commit_text(false, cx);
    }
    fn toggle_commit_amend(&mut self, cx: &mut Context<Self>) {
        self.commit_amend = !self.commit_amend;
        cx.notify();
    }
    fn toggle_commit_sign_off(&mut self, cx: &mut Context<Self>) {
        self.commit_sign_off = !self.commit_sign_off;
        cx.notify();
    }

    fn prompt_commit_text(&mut self, subject: bool, cx: &mut Context<Self>) {
        self.activity = if subject {
            "Enter commit subject…"
        } else {
            "Enter commit body…"
        }
        .into();
        cx.spawn(async move |this, cx| {
            let text = cx
                .background_spawn(async move {
                    let label = if subject {
                        "Commit subject"
                    } else {
                        "Commit body"
                    };
                    Command::new("osascript")
                        .args([
                            "-e",
                            &format!(
                                "text returned of (display dialog \"{label}\" default answer \"\")"
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
                })
                .await;
            let _ = this.update(cx, |app, cx| {
                if let Some(text) = text {
                    if subject {
                        app.commit_subject = text;
                    } else {
                        app.commit_body = text;
                    }
                    app.activity = "Commit draft updated.".into();
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn commit_draft(&mut self, cx: &mut Context<Self>) {
        if self.mutation_in_flight
            || self.commit_subject.trim().is_empty()
            || self.status_groups().staged.is_empty()
        {
            return;
        }
        let ShellState::Repository(repository) = &self.state else {
            return;
        };
        let repository = repository.clone();
        let worker_repository = repository.clone();
        let request = CommitRequest {
            subject: self.commit_subject.clone(),
            body: self.commit_body.clone(),
            amend: self.commit_amend,
            sign_off: self.commit_sign_off,
        };
        self.mutation_in_flight = true;
        self.activity = "Committing…".into();
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
                        app.activity = "Commit complete.".into();
                        app.load_working_copy(repository.clone(), cx);
                        app.history_reveal_oid = Some(oid);
                        app.navigate_to(RepositoryView::History, cx);
                        app.reset_history();
                        app.load_history(repository, None, cx);
                    }
                    Err(error) => app.activity = git_failure_message("Commit", &error),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn run_mutation(&mut self, operation: Mutation, paths: Vec<GitPath>, cx: &mut Context<Self>) {
        let ShellState::Repository(repository) = &self.state else {
            return;
        };
        let repository = repository.clone();
        self.mutation_in_flight = true;
        self.activity = format!("{}…", operation.label());
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
                        app.selected_paths.clear();
                        app.loaded_diff = None;
                        app.selected_diff = None;
                        app.selected_diff_lines.clear();
                        app.activity = format!("{} complete.", operation.label());
                        app.load_working_copy(repository, cx);
                    }
                    Err(error) => app.activity = git_failure_message(operation.label(), &error),
                }
                cx.notify();
            });
        })
        .detach();
    }
}
