//! macOS application entry point.

use std::{
    collections::BTreeSet,
    ffi::OsString,
    fs,
    os::unix::ffi::OsStringExt,
    path::{Path, PathBuf},
    process::Command,
    sync::{
        Arc, Mutex,
        mpsc::{self, Receiver},
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

mod actions;
mod keymap;
mod menus;

use app_core::{RecentRepositoryStore, RepositoryOpenError, WindowGeometry, open_repository};
use git_cli::{CommitRequest, GitExecutable, GitStatusError, LoadedDiff, read_stderr_limited};
use git_domain::{
    GitPath, GraphRow, GraphState, HeadStatus, HistoryCommit, HistoryPage, HistoryReference,
    HistoryRequest, NamedRef, RefDecoration, RefSnapshot, StatusEntry, WorktreeRepository,
    WorktreeStatus, layout_history_graph,
};
use gpui::{
    App, Application, Bounds, ClickEvent, ClipboardItem, Context, ExternalPaths, FocusHandle,
    IntoElement, ListAlignment, ListState, MouseButton, PathBuilder, PathPromptOptions, Render,
    Subscription, Window, WindowAppearance, WindowBounds, WindowOptions, canvas, div, list, point,
    prelude::*, px, size,
};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use ui_kit::{Appearance, Theme};

use actions::{
    CommandPalette, FocusComposer, HistoryNext, HistoryPrevious, NavigateBack, NavigateForward,
    OpenRepository, Refresh, ShortcutReference, ToggleAppearance, WidenInspector, WidenSidebar,
};

const INITIAL_WINDOW_SIZE: (f32, f32) = (1200.0, 800.0);
const MINIMUM_WINDOW_SIZE: (f32, f32) = (800.0, 560.0);
const MINIMUM_PANE_WIDTH: f32 = 180.0;
const MAXIMUM_PANE_WIDTH: f32 = 440.0;
const MINIMUM_CONTENT_WIDTH: f32 = 360.0;

fn network_failure_message(label: &str, error: &str) -> String {
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

fn git_failure_message(label: &str, error: &str) -> String {
    if error.to_lowercase().contains("index.lock") {
        format!(
            "{label} could not run because Git's index is locked. Check that no Git process is still running; if none is, inspect .git/index.lock before removing it manually."
        )
    } else {
        format!("{label} failed: {error}")
    }
}

fn repository_is_available(repository: &WorktreeRepository) -> bool {
    repository.worktree_root.is_dir() && repository.git_dir.is_dir()
}

fn repository_unavailable_message(repository: &WorktreeRepository) -> String {
    format!(
        "{} is no longer available. Restore the repository folder, then open it again.",
        repository.worktree_root.display()
    )
}

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LastAction {
    Refresh,
}

struct OpenedRepository {
    repository: WorktreeRepository,
    recents: Vec<PathBuf>,
}

enum ShellState {
    Welcome,
    Loading(PathBuf),
    Repository(WorktreeRepository),
    Error(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ThemeMode {
    System,
    Light,
    Dark,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RepositoryView {
    WorkingCopy,
    History,
}

struct NetworkOperation {
    child: Option<git_cli::GitChild>,
    cancelled: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ForcePushState {
    Idle,
    AwaitingConfirmation,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ShortcutReferenceState {
    Hidden,
    Visible,
}

#[derive(Clone)]
enum RefContext {
    LocalBranch(String),
    RemoteBranch(String),
    Tag(String),
    Remote(String),
}

#[derive(Clone, Copy)]
enum RefKind {
    LocalBranch,
    RemoteBranch,
    Tag,
}

impl RefKind {
    fn context(self, name: String) -> RefContext {
        match self {
            Self::LocalBranch => RefContext::LocalBranch(name),
            Self::RemoteBranch => RefContext::RemoteBranch(name),
            Self::Tag => RefContext::Tag(name),
        }
    }
}

struct GitronimoApp {
    focus_handle: FocusHandle,
    last_action: Option<LastAction>,
    appearance: Appearance,
    theme_mode: ThemeMode,
    sidebar_width: f32,
    inspector_width: f32,
    state: ShellState,
    recents: Vec<PathBuf>,
    activity: String,
    working_copy: Option<WorktreeStatus>,
    refs: RefSnapshot,
    expanded_ref_groups: BTreeSet<String>,
    ref_context: Option<RefContext>,
    selected_paths: Vec<GitPath>,
    context_path: Option<GitPath>,
    loaded_diff: Option<LoadedDiff>,
    selected_diff: Option<(GitPath, bool)>,
    pending_discard: Option<Vec<GitPath>>,
    pending_stash_action: Option<StashAction>,
    pending_branch_delete: Option<String>,
    force_push_state: ForcePushState,
    shortcut_reference_state: ShortcutReferenceState,
    commit_subject: String,
    commit_body: String,
    commit_amend: bool,
    commit_sign_off: bool,
    author_identity: String,
    repository_view: RepositoryView,
    navigation_back: Vec<RepositoryView>,
    navigation_forward: Vec<RepositoryView>,
    history: Vec<HistoryCommit>,
    history_rows: Vec<GraphRow>,
    history_state: GraphState,
    history_reference: HistoryReference,
    history_next: Option<String>,
    history_decorations: Vec<RefDecoration>,
    selected_history: Option<usize>,
    history_search: String,
    history_list_state: ListState,
    history_paths: Vec<GitPath>,
    history_diff: Option<LoadedDiff>,
    history_selection_token: u64,
    history_load_token: u64,
    mutation_in_flight: bool,
    network_operation: Option<Arc<Mutex<NetworkOperation>>>,
    watcher: Option<RecommendedWatcher>,
    watch_events: Option<Receiver<()>>,
    store: RecentRepositoryStore,
    diagnostics: String,
    subscriptions: Vec<Subscription>,
}

impl GitronimoApp {
    fn has_commit_draft(&self) -> bool {
        !self.commit_subject.trim().is_empty() || !self.commit_body.trim().is_empty()
    }

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
            refs: RefSnapshot::default(),
            expanded_ref_groups,
            ref_context: None,
            selected_paths: Vec::new(),
            context_path: None,
            loaded_diff: None,
            selected_diff: None,
            pending_discard: None,
            pending_stash_action: None,
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
            refs: RefSnapshot::default(),
            expanded_ref_groups,
            ref_context: None,
            selected_paths: Vec::new(),
            context_path: None,
            loaded_diff: None,
            selected_diff: None,
            pending_discard: None,
            pending_stash_action: None,
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
                self.pending_discard = None;
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
                        .args(["-e", "choose from list {\"Refresh working copy\", \"Show history\", \"Show working copy\", \"Show keyboard shortcuts\"} with title \"Gitronimo commands\" with prompt \"Choose an action\""])
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
        self.move_history_selection(-1, cx);
    }

    fn history_next(&mut self, _: &HistoryNext, _: &mut Window, cx: &mut Context<Self>) {
        self.move_history_selection(1, cx);
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

    fn change_history_reference(
        &mut self,
        reference: HistoryReference,
        repository: WorktreeRepository,
        cx: &mut Context<Self>,
    ) {
        self.history_reference = reference;
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
        self.load_history(repository, None, cx);
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
                    }
                    Err(error) => app.activity = format!("History load failed: {error}"),
                }
                cx.notify();
            });
        }).detach();
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
                    GitExecutable::discover()
                        .map_err(|error| error.to_string())?
                        .commit(&worker_repository, &request)
                        .map_err(|error| format!("{error:?}"))
                })
                .await;
            let _ = this.update(cx, |app, cx| {
                app.mutation_in_flight = false;
                match result {
                    Ok(()) => {
                        app.commit_subject.clear();
                        app.commit_body.clear();
                        app.activity = "Commit complete.".into();
                        app.load_working_copy(repository, cx);
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

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mutation {
    StageSelected,
    UnstageSelected,
    StageAll,
    UnstageAll,
    DiscardSelected,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum StashAction {
    Pop,
    Drop,
}

impl Mutation {
    fn needs_paths(self) -> bool {
        matches!(
            self,
            Self::StageSelected | Self::UnstageSelected | Self::DiscardSelected
        )
    }
    fn label(self) -> &'static str {
        match self {
            Self::StageSelected => "Stage selected",
            Self::UnstageSelected => "Unstage selected",
            Self::StageAll => "Stage all",
            Self::UnstageAll => "Unstage all",
            Self::DiscardSelected => "Discard selected",
        }
    }
}

fn discard_selected(
    git: &GitExecutable,
    repository: &WorktreeRepository,
    paths: &[GitPath],
) -> Result<(), GitStatusError> {
    for path in paths {
        match git.discard_tracked_paths(repository, std::slice::from_ref(path)) {
            Ok(()) => {}
            Err(GitStatusError::UntrackedDeletionRefused) => {
                move_to_trash(&repository.worktree_root, path).map_err(GitStatusError::Io)?;
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

fn eligible_trash_path(root: &Path, path: &GitPath) -> std::io::Result<PathBuf> {
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

fn appearance_from_window(appearance: WindowAppearance) -> Appearance {
    match appearance {
        WindowAppearance::Light | WindowAppearance::VibrantLight => Appearance::Light,
        WindowAppearance::Dark | WindowAppearance::VibrantDark => Appearance::Dark,
    }
}

fn window_title(state: &ShellState, has_commit_draft: bool) -> String {
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

impl Render for GitronimoApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        window.set_window_title(&window_title(&self.state, self.has_commit_draft()));
        let colors = Theme::for_appearance(self.appearance).colors;
        let sidebar_width = self.sidebar_width;
        let inspector_width = self.inspector_width;
        let show_inspector = !matches!(self.state, ShellState::Welcome)
            && shows_inspector(
                f32::from(window.viewport_size().width),
                sidebar_width,
                inspector_width,
            );
        let content = match &self.state {
            ShellState::Welcome => self.welcome_view(&colors, cx).into_any_element(),
            ShellState::Loading(path) => loading_view(path, &colors).into_any_element(),
            ShellState::Repository(repository) => self
                .repository_view(repository, &colors, cx)
                .into_any_element(),
            ShellState::Error(message) => error_view(message, &colors).into_any_element(),
        };

        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(colors.window_background)
            .text_color(colors.text_primary)
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::refresh))
            .on_action(cx.listener(Self::focus_composer))
            .on_action(cx.listener(Self::show_command_palette))
            .on_action(cx.listener(Self::toggle_shortcut_reference))
            .on_action(cx.listener(Self::history_previous))
            .on_action(cx.listener(Self::history_next))
            .on_action(cx.listener(Self::navigate_back))
            .on_action(cx.listener(Self::navigate_forward))
            .on_action(cx.listener(Self::toggle_appearance))
            .on_action(cx.listener(Self::widen_sidebar))
            .on_action(cx.listener(Self::widen_inspector))
            .on_drop(cx.listener(Self::dropped_paths))
            .child(self.workspace_toolbar(&colors, cx))
            .child(
                div()
                    .flex_1()
                    .flex()
                    .child(self.sidebar_view(sidebar_width, &colors, cx))
                    .child(
                        div()
                            .flex_1()
                            .h_full()
                            .p_6()
                            .child(content)
                            .children(self.shortcut_reference_view(&colors, cx)),
                    )
                    .when(show_inspector, |this| {
                        this.child(
                            div()
                                .w(px(inspector_width))
                                .h_full()
                                .p_4()
                                .bg(colors.panel_background)
                                .border_l_1()
                                .border_color(colors.border)
                                .child("Diagnostics")
                                .child(self.diagnostics.clone())
                                .child("One repository window opens per selection."),
                        )
                    }),
            )
            .child(
                div()
                    .min_h(px(30.0))
                    .px_4()
                    .flex()
                    .items_center()
                    .bg(colors.raised_background)
                    .border_t_1()
                    .border_color(colors.border)
                    .text_color(activity_color(&self.activity, &colors))
                    .child(activity_label(&self.activity)),
            )
    }
}

impl GitronimoApp {
    fn workspace_toolbar(
        &self,
        colors: &ui_kit::ThemeColors,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let context = match &self.state {
            ShellState::Welcome => "Local Git workspace".to_owned(),
            ShellState::Loading(_) => "Opening repository".to_owned(),
            ShellState::Repository(repository) => repository
                .worktree_root
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("Repository")
                .to_owned(),
            ShellState::Error(_) => "Repository needs attention".to_owned(),
        };
        div()
            .min_h(px(52.0))
            .px_5()
            .flex()
            .items_center()
            .justify_between()
            .bg(colors.panel_background)
            .border_b_1()
            .border_color(colors.border)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(div().text_lg().child("Gitronimo"))
                    .child(div().text_color(colors.text_secondary).child(context)),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(window_action_button(
                        "Command palette",
                        colors,
                        cx,
                        |_, window, cx| {
                            window.dispatch_action(Box::new(CommandPalette), cx);
                        },
                    ))
                    .child(primary_window_action_button(
                        "Open repository",
                        colors,
                        cx,
                        |_, window, cx| {
                            window.dispatch_action(Box::new(OpenRepository), cx);
                        },
                    )),
            )
    }

    fn sidebar_view(
        &self,
        width: f32,
        colors: &ui_kit::ThemeColors,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        if matches!(self.state, ShellState::Welcome) {
            return welcome_sidebar_view(width, colors);
        }
        let groups = self.status_groups();
        let branch = self.working_copy.as_ref().map_or_else(
            || "Branch: loading…".to_owned(),
            |status| match &status.branch.head {
                HeadStatus::Branch(name) => format!("Branch: {}", String::from_utf8_lossy(&name.0)),
                HeadStatus::Detached => "Branch: detached HEAD".into(),
                HeadStatus::Unborn => "Branch: unborn".into(),
                HeadStatus::Unknown => "Branch: unknown".into(),
            },
        );
        let upstream = self.working_copy.as_ref().and_then(|status| {
            status.branch.upstream.as_ref().map(|upstream| {
                format!(
                    "Upstream: {} (+{}/-{})",
                    String::from_utf8_lossy(&upstream.0),
                    status.branch.ahead,
                    status.branch.behind
                )
            })
        });
        div()
            .w(px(width))
            .h_full()
            .p_4()
            .flex()
            .flex_col()
            .gap_2()
            .bg(colors.sidebar_background)
            .border_r_1()
            .border_color(colors.border)
            .child("Workspace")
            .child(branch)
            .children(upstream)
            .child("Working Copy")
            .child(status_badge("Staged", groups.staged.len(), colors))
            .child(status_badge("Unstaged", groups.unstaged.len(), colors))
            .child(status_badge("Untracked", groups.untracked.len(), colors))
            .child(status_badge("Conflicts", groups.conflicts.len(), colors))
            .child("History")
            .child("Local branches")
            .children(self.ref_rows(
                "local",
                &self.refs.local_branches,
                RefKind::LocalBranch,
                colors,
                cx,
            ))
            .child("Remote branches")
            .children(self.ref_rows(
                "remote",
                &self.refs.remote_branches,
                RefKind::RemoteBranch,
                colors,
                cx,
            ))
            .child("Tags")
            .children(self.ref_rows("tag", &self.refs.tags, RefKind::Tag, colors, cx))
            .child("Remotes")
            .children(
                self.refs
                    .remotes
                    .iter()
                    .enumerate()
                    .filter_map(|(index, remote)| {
                        String::from_utf8(remote.name.0.clone()).ok().map(|name| {
                            let context = RefContext::Remote(name.clone());
                            div()
                                .id(("remote-ref", index))
                                .pl_2()
                                .cursor_pointer()
                                .on_click(cx.listener(move |app, _, _, cx| {
                                    app.select_ref_context(context.clone(), cx);
                                }))
                                .child(name)
                        })
                    }),
            )
            .into_any_element()
    }

    fn shortcut_reference_view(
        &self,
        colors: &ui_kit::ThemeColors,
        cx: &mut Context<Self>,
    ) -> Option<gpui::AnyElement> {
        (self.shortcut_reference_state == ShortcutReferenceState::Visible).then(|| {
            div()
                .mt_4()
                .p_3()
                .flex()
                .flex_col()
                .gap_1()
                .bg(colors.raised_background)
                .border_1()
                .border_color(colors.border)
                .child("Keyboard shortcuts")
                .child("Command-O  Open repository")
                .child("Command-R  Refresh working copy")
                .child("Command-Shift-C  Edit commit subject")
                .child("Command-Shift-P  Command palette")
                .child("Command-/  Show or hide this reference")
                .child("Command-[ / Command-]  Back / Forward")
                .child("Up / Down  Move through loaded history")
                .child(file_action_button(
                    "Hide shortcut reference",
                    colors,
                    cx,
                    |app, cx| {
                        app.shortcut_reference_state = ShortcutReferenceState::Hidden;
                        cx.notify();
                    },
                ))
                .into_any_element()
        })
    }

    fn ref_rows(
        &self,
        category: &str,
        refs: &[NamedRef],
        kind: RefKind,
        colors: &ui_kit::ThemeColors,
        cx: &mut Context<Self>,
    ) -> Vec<gpui::AnyElement> {
        let mut groups = BTreeSet::new();
        let mut rows = Vec::new();
        let id_prefix = match category {
            "local" => "local-ref",
            "remote" => "remote-branch-ref",
            _ => "tag-ref",
        };
        let group_id_prefix = match category {
            "local" => "local-ref-group",
            "remote" => "remote-ref-group",
            _ => "tag-ref-group",
        };
        for reference in refs {
            let Ok(name) = String::from_utf8(reference.name.0.clone()) else {
                continue;
            };
            let parts: Vec<_> = name.split('/').collect();
            let mut visible = true;
            for depth in 1..parts.len() {
                let group = parts[..depth].join("/");
                let key = format!("{category}:{group}");
                let expanded = self.expanded_ref_groups.contains(&key);
                if groups.insert(key.clone()) {
                    let label = format!(
                        "{}{} {}",
                        "  ".repeat(depth),
                        if expanded { "⌄" } else { "›" },
                        group.rsplit('/').next().unwrap_or_default()
                    );
                    rows.push(
                        div()
                            .id((group_id_prefix, rows.len()))
                            .text_color(colors.text_secondary)
                            .cursor_pointer()
                            .on_click(cx.listener(move |app, _, _, cx| {
                                app.toggle_ref_group(key.clone(), cx);
                            }))
                            .child(label)
                            .into_any_element(),
                    );
                }
                visible &= expanded;
                if !visible {
                    break;
                }
            }
            if visible {
                let context = kind.context(name.clone());
                let indent = u16::try_from(parts.len().saturating_mul(12)).unwrap_or(u16::MAX);
                rows.push(
                    div()
                        .id((id_prefix, rows.len()))
                        .pl(px(f32::from(indent)))
                        .cursor_pointer()
                        .on_click(cx.listener(move |app, _, _, cx| {
                            app.select_ref_context(context.clone(), cx);
                        }))
                        .child(parts.last().copied().unwrap_or_default().to_owned())
                        .into_any_element(),
                );
            }
        }
        rows
    }

    #[allow(clippy::too_many_lines)]
    fn repository_view(
        &self,
        repository: &WorktreeRepository,
        colors: &ui_kit::ThemeColors,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        if self.repository_view == RepositoryView::History {
            return self.history_view(repository, colors, cx).into_any_element();
        }
        let groups = self.status_groups();
        let has_local_branches = !self.refs.local_branches.is_empty();
        let has_remotes = !self.refs.remotes.is_empty();
        let has_upstream = self.has_upstream();
        let has_attached_branch = self.has_attached_branch();
        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .p_4()
                    .flex()
                    .items_center()
                    .justify_between()
                    .bg(colors.panel_background)
                    .border_1()
                    .border_color(colors.border)
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(div().text_xl().child("Working copy"))
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(colors.text_secondary)
                                    .child(repository.worktree_root.display().to_string()),
                            ),
                    )
                    .child(file_action_button("History", colors, cx, {
                        let repository = repository.clone();
                        move |app, cx| app.show_history(repository.clone(), cx)
                    })),
            )
            .children(self.navigation_controls(colors, cx))
            .child(workspace_section(
                "Branch",
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(div().flex().gap_2().children([
                        validated_action_button(
                            "Checkout branch…",
                            has_local_branches,
                            "No local branches are available.",
                            colors,
                            cx,
                            |_, cx| GitronimoApp::prompt_branch_name(false, cx),
                        ),
                        file_action_button("New branch from HEAD…", colors, cx, |_, cx| {
                            GitronimoApp::prompt_branch_name(true, cx);
                        }),
                    ]))
                    .child(div().flex().gap_2().children([
                        validated_action_button(
                            "Rename current branch…",
                            has_attached_branch,
                            "Checkout a local branch first.",
                            colors,
                            cx,
                            |_, cx| GitronimoApp::prompt_rename_current_branch(cx),
                        ),
                        validated_action_button(
                            "Delete local branch…",
                            has_local_branches,
                            "No local branches are available.",
                            colors,
                            cx,
                            |_, cx| GitronimoApp::prompt_delete_local_branch(cx),
                        ),
                    ])),
                colors,
            ))
            .child(workspace_section(
                "Sync",
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(div().flex().gap_2().children([
                        validated_action_button(
                            "Fetch default remote",
                            has_remotes,
                            "Add a remote before fetching.",
                            colors,
                            cx,
                            GitronimoApp::fetch_default_remote,
                        ),
                        validated_action_button(
                            "Fetch remote…",
                            has_remotes,
                            "Add a remote before fetching.",
                            colors,
                            cx,
                            |_, cx| GitronimoApp::prompt_fetch_remote(cx),
                        ),
                    ]))
                    .child(div().flex().gap_2().children([
                        validated_action_button(
                            "Pull current branch",
                            has_upstream,
                            "Set an upstream before pulling.",
                            colors,
                            cx,
                            GitronimoApp::pull_current,
                        ),
                        validated_action_button(
                            "Push current branch",
                            has_upstream,
                            "Set an upstream before pushing.",
                            colors,
                            cx,
                            GitronimoApp::push_current,
                        ),
                    ]))
                    .child(div().flex().gap_2().children([
                        validated_action_button(
                            "Publish current branch",
                            has_remotes && has_attached_branch,
                            "Checkout a branch and add a remote before publishing.",
                            colors,
                            cx,
                            GitronimoApp::publish_current,
                        ),
                        validated_action_button(
                            "Advanced force-with-lease…",
                            has_upstream,
                            "Set an upstream before force-with-lease is available.",
                            colors,
                            cx,
                            GitronimoApp::request_force_with_lease,
                        ),
                    ]))
                    .children(self.network_cancel_button(colors, cx)),
                colors,
            ))
            .children(self.ref_context_menu_view(colors, cx))
            .children(self.working_copy.as_ref().is_none().then(|| {
                state_panel(
                    "Loading working copy",
                    "Reading status, branches, and remotes in the background.",
                    colors.warning,
                    colors,
                )
            }))
            .child(self.mutation_controls(colors, cx))
            .children(self.discard_confirmation_view(colors, cx))
            .children(self.stash_pop_confirmation_view(colors, cx))
            .children(self.stash_drop_confirmation_view(colors, cx))
            .children(self.branch_delete_confirmation_view(colors, cx))
            .children(self.force_with_lease_confirmation_view(colors, cx))
            .child(self.commit_composer_view(colors, cx))
            .children(self.context_menu_view(repository, colors, cx))
            .child(self.status_group_view("Staged", &groups.staged, true, colors, cx))
            .child(self.status_group_view("Unstaged", &groups.unstaged, false, colors, cx))
            .child(self.status_group_view("Untracked", &groups.untracked, false, colors, cx))
            .child(self.status_group_view("Conflicts", &groups.conflicts, false, colors, cx))
            .children(self.diff_view(colors, cx))
            .into_any_element()
    }

    fn history_row_count(&self) -> usize {
        let search = self.history_search.to_lowercase();
        self.history
            .iter()
            .filter(|commit| {
                search.is_empty()
                    || commit.oid.contains(&search)
                    || String::from_utf8_lossy(&commit.subject)
                        .to_lowercase()
                        .contains(&search)
                    || String::from_utf8_lossy(&commit.author.name)
                        .to_lowercase()
                        .contains(&search)
            })
            .count()
    }

    fn ref_context_menu_view(
        &self,
        colors: &ui_kit::ThemeColors,
        cx: &mut Context<Self>,
    ) -> Option<gpui::AnyElement> {
        let context = self.ref_context.clone()?;
        let (title, reference) = match &context {
            RefContext::LocalBranch(name) => ("Local branch", name.clone()),
            RefContext::RemoteBranch(name) => ("Remote branch", name.clone()),
            RefContext::Tag(name) => ("Tag", name.clone()),
            RefContext::Remote(name) => ("Remote", name.clone()),
        };
        let mut menu = div()
            .p_2()
            .flex()
            .flex_col()
            .gap_1()
            .bg(colors.raised_background)
            .border_1()
            .border_color(colors.border)
            .child(format!("{title}: {reference}"));
        match context {
            RefContext::LocalBranch(branch) => {
                let checkout = branch.clone();
                let history = branch.clone();
                menu = menu
                    .child(file_action_button(
                        "Checkout branch",
                        colors,
                        cx,
                        move |app, cx| {
                            app.checkout_branch(checkout.clone(), cx);
                        },
                    ))
                    .child(file_action_button(
                        "View branch history",
                        colors,
                        cx,
                        move |app, cx| {
                            app.show_ref_history(history.clone(), cx);
                        },
                    ));
            }
            RefContext::RemoteBranch(branch) | RefContext::Tag(branch) => {
                let create_start = branch.clone();
                let history = branch.clone();
                menu = menu
                    .child(file_action_button(
                        "New branch from ref…",
                        colors,
                        cx,
                        move |_, cx| {
                            GitronimoApp::prompt_branch_from_ref(create_start.clone(), cx);
                        },
                    ))
                    .child(file_action_button(
                        "View ref history",
                        colors,
                        cx,
                        move |app, cx| {
                            app.show_ref_history(history.clone(), cx);
                        },
                    ));
            }
            RefContext::Remote(remote) => {
                menu = menu.child(file_action_button(
                    "Fetch this remote",
                    colors,
                    cx,
                    move |app, cx| {
                        app.run_network_command(
                            format!("Fetching {remote}"),
                            vec!["fetch".into(), "--progress".into(), remote.clone().into()],
                            cx,
                        );
                    },
                ));
            }
        }
        Some(menu.into_any_element())
    }

    fn navigation_controls(
        &self,
        colors: &ui_kit::ThemeColors,
        cx: &mut Context<Self>,
    ) -> Option<gpui::AnyElement> {
        (!self.navigation_back.is_empty() || !self.navigation_forward.is_empty()).then(|| {
            div()
                .flex()
                .gap_2()
                .children(self.navigation_back.last().map(|_| {
                    file_action_button("Back", colors, cx, |app, cx| {
                        if let Some(view) = app.navigation_back.pop() {
                            app.navigation_forward.push(app.repository_view);
                            app.repository_view = view;
                            cx.notify();
                        }
                    })
                }))
                .children(self.navigation_forward.last().map(|_| {
                    file_action_button("Forward", colors, cx, |app, cx| {
                        if let Some(view) = app.navigation_forward.pop() {
                            app.navigation_back.push(app.repository_view);
                            app.repository_view = view;
                            cx.notify();
                        }
                    })
                }))
                .into_any_element()
        })
    }

    #[allow(clippy::too_many_lines)]
    fn history_view(
        &self,
        repository: &WorktreeRepository,
        colors: &ui_kit::ThemeColors,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let search = self.history_search.to_lowercase();
        let rows = self
            .history
            .iter()
            .enumerate()
            .filter(|(_, commit)| {
                search.is_empty()
                    || commit.oid.contains(&search)
                    || String::from_utf8_lossy(&commit.subject)
                        .to_lowercase()
                        .contains(&search)
                    || String::from_utf8_lossy(&commit.author.name)
                        .to_lowercase()
                        .contains(&search)
            })
            .map(|(history_index, commit)| {
                let graph_row = self.history_rows.get(history_index);
                let lane = graph_row.map_or(0, |row| row.lane);
                let parent_lanes = graph_row.map_or_else(Vec::new, |row| row.parent_lanes.clone());
                let decorations = self
                    .history_decorations
                    .iter()
                    .filter(|decoration| decoration.target == commit.oid)
                    .map(|decoration| String::from_utf8_lossy(&decoration.name).to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                (
                    history_index,
                    lane,
                    parent_lanes,
                    format!(
                        "{} ● {} · {} — {} {}",
                        "│ ".repeat(lane),
                        String::from_utf8_lossy(&commit.author.name),
                        commit.author.timestamp,
                        String::from_utf8_lossy(&commit.subject),
                        decorations
                    ),
                )
            })
            .collect::<Vec<_>>();
        let selected = self.selected_history;
        let list_colors = *colors;
        let list_repository = repository.clone();
        let rows = list(
            self.history_list_state.clone(),
            cx.processor(move |_app, visible_index: usize, _, cx| {
                let (history_index, lane, parent_lanes, label) = rows[visible_index].clone();
                let repository = list_repository.clone();
                div()
                    .id(visible_index)
                    .h(px(28.0))
                    .px_2()
                    .flex()
                    .items_center()
                    .bg(if selected == Some(history_index) {
                        list_colors.raised_background
                    } else {
                        list_colors.panel_background
                    })
                    .border_b_1()
                    .border_color(list_colors.border)
                    .child(
                        canvas(
                            move |bounds, _, _| {
                                let lane_offset = u8::try_from(lane.min(100)).unwrap_or(100);
                                let x = bounds.origin.x + px(10.0 + f32::from(lane_offset) * 8.0);
                                let mut path = PathBuilder::stroke(px(2.0));
                                path.move_to(point(x, bounds.origin.y));
                                let center_y = bounds.origin.y + bounds.size.height / 2.0;
                                path.line_to(point(x, center_y));
                                for parent_lane in &parent_lanes {
                                    let parent_offset =
                                        u8::try_from((*parent_lane).min(100)).unwrap_or(100);
                                    let parent_x =
                                        bounds.origin.x + px(10.0 + f32::from(parent_offset) * 8.0);
                                    path.move_to(point(x, center_y));
                                    path.line_to(point(
                                        parent_x,
                                        bounds.origin.y + bounds.size.height,
                                    ));
                                }
                                if parent_lanes.is_empty() {
                                    path.line_to(point(x, bounds.origin.y + bounds.size.height));
                                }
                                path.build().ok()
                            },
                            move |_, path, window, _| {
                                if let Some(path) = path {
                                    window.paint_path(
                                        path,
                                        list_colors.graph_lanes
                                            [lane % list_colors.graph_lanes.len()],
                                    );
                                }
                            },
                        )
                        .w(px(28.0))
                        .h_full(),
                    )
                    .cursor_pointer()
                    .on_click(cx.listener(move |app, _: &ClickEvent, _, cx| {
                        app.select_history_commit(history_index, repository.clone(), cx);
                    }))
                    .child(label)
                    .into_any_element()
            }),
        )
        .h(px(360.0));
        let inspector = self
            .selected_history
            .and_then(|index| self.history.get(index))
            .map(|commit| {
                div()
                    .p_2()
                    .border_1()
                    .border_color(colors.border)
                    .child(format!(
                        "{}\n{}\n{}\nChanged: {}",
                        commit.oid,
                        String::from_utf8_lossy(&commit.body),
                        commit.parents.join(" "),
                        self.history_paths
                            .iter()
                            .map(|path| String::from_utf8_lossy(&path.0))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ))
                    .children(self.history_diff.as_ref().map(|diff| {
                        div().child(format!(
                            "Selected diff: {} file(s){}",
                            diff.diff.files.len(),
                            if diff.truncated { " (truncated)" } else { "" }
                        ))
                    }))
                    .into_any_element()
            });
        let load_more = self.history_next.as_ref().map(|before| {
            let repository = repository.clone();
            let before = before.clone();
            file_action_button("Load more history", colors, cx, move |app, cx| {
                app.load_history(repository.clone(), Some(before.clone()), cx);
            })
        });
        let current_repository = repository.clone();
        let all_repository = repository.clone();
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(div().text_xl().child("History"))
            .child(file_action_button("Working Copy", colors, cx, |app, cx| {
                app.navigate_to(RepositoryView::WorkingCopy, cx);
            }))
            .child(file_action_button(
                "Current branch",
                colors,
                cx,
                move |app, cx| {
                    app.change_history_reference(
                        HistoryReference::Current,
                        current_repository.clone(),
                        cx,
                    );
                },
            ))
            .child(file_action_button(
                "All refs",
                colors,
                cx,
                move |app, cx| {
                    app.change_history_reference(HistoryReference::All, all_repository.clone(), cx);
                },
            ))
            .child(file_action_button(
                "Branch or tag…",
                colors,
                cx,
                |_, cx| GitronimoApp::prompt_history_reference(cx),
            ))
            .child(format!(
                "Search: {}",
                if self.history_search.is_empty() {
                    "(all loaded commits)"
                } else {
                    &self.history_search
                }
            ))
            .child(file_action_button("Search history", colors, cx, |_, cx| {
                GitronimoApp::prompt_history_search(cx);
            }))
            .child(file_action_button("Reveal HEAD", colors, cx, |app, cx| {
                app.reveal_history_head(cx);
            }))
            .child(file_action_button(
                "Copy selected OID",
                colors,
                cx,
                GitronimoApp::copy_selected_history_oid,
            ))
            .child(file_action_button(
                "New branch from selected commit…",
                colors,
                cx,
                |_, cx| GitronimoApp::prompt_branch_from_selected(cx),
            ))
            .child(rows)
            .children(load_more)
            .children(inspector)
    }

    fn status_groups(&self) -> StatusGroups<'_> {
        let mut groups = StatusGroups::default();
        let Some(status) = &self.working_copy else {
            return groups;
        };
        for entry in &status.entries {
            match entry {
                StatusEntry::Unmerged { .. } => groups.conflicts.push(entry),
                StatusEntry::Untracked(_) => groups.untracked.push(entry),
                StatusEntry::Ignored(_) => {}
                StatusEntry::Ordinary { status, .. } | StatusEntry::Renamed { status, .. } => {
                    if status.0[0] != b'.' {
                        groups.staged.push(entry);
                    }
                    if status.0[1] != b'.' {
                        groups.unstaged.push(entry);
                    }
                }
            }
        }
        groups
    }

    fn mutation_controls(
        &self,
        colors: &ui_kit::ThemeColors,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let disabled = self.mutation_in_flight;
        workspace_section(
            "Changes",
            div().flex().gap_2().children([
                mutation_button(
                    "Stage selected",
                    disabled,
                    Mutation::StageSelected,
                    colors,
                    cx,
                ),
                mutation_button(
                    "Unstage selected",
                    disabled,
                    Mutation::UnstageSelected,
                    colors,
                    cx,
                ),
                mutation_button("Stage all", disabled, Mutation::StageAll, colors, cx),
                mutation_button("Unstage all", disabled, Mutation::UnstageAll, colors, cx),
                mutation_button(
                    "Discard selected",
                    disabled,
                    Mutation::DiscardSelected,
                    colors,
                    cx,
                ),
                file_action_button("Stash tracked changes", colors, cx, |app, cx| {
                    app.create_stash(false, cx);
                }),
                file_action_button("Stash including untracked", colors, cx, |app, cx| {
                    app.create_stash(true, cx);
                }),
                file_action_button("Apply latest stash", colors, cx, |app, cx| {
                    app.apply_latest_stash(cx);
                }),
                file_action_button("Pop latest stash", colors, cx, |app, cx| {
                    app.pending_stash_action = Some(StashAction::Pop);
                    app.activity =
                        "Confirm before removing the latest stash recovery entry.".into();
                    cx.notify();
                }),
                file_action_button("Drop latest stash", colors, cx, |app, cx| {
                    app.pending_stash_action = Some(StashAction::Drop);
                    app.activity = "Confirm before permanently removing the latest stash.".into();
                    cx.notify();
                }),
            ]),
            colors,
        )
    }

    fn discard_confirmation_view(
        &self,
        colors: &ui_kit::ThemeColors,
        cx: &mut Context<Self>,
    ) -> Option<gpui::AnyElement> {
        self.pending_discard.as_ref().map(|paths| {
            div()
                .p_2()
                .bg(colors.raised_background)
                .border_1()
                .border_color(colors.border)
                .child(format!(
                    "Discard {} path(s)? Tracked changes restore from HEAD; untracked files move to Trash.",
                    paths.len()
                ))
                .child(file_action_button("Confirm discard", colors, cx, |app, cx| {
                    app.confirm_discard(cx);
                }))
                .into_any_element()
        })
    }

    fn stash_pop_confirmation_view(
        &self,
        colors: &ui_kit::ThemeColors,
        cx: &mut Context<Self>,
    ) -> Option<gpui::AnyElement> {
        (self.pending_stash_action == Some(StashAction::Pop)).then(|| {
            div()
                .p_2()
                .bg(colors.raised_background)
                .border_1()
                .border_color(colors.border)
                .child("Pop the latest stash? Its recovery entry will be removed after a successful apply.")
                .child(file_action_button("Confirm pop latest stash", colors, cx, |app, cx| {
                    app.pop_latest_stash(cx);
                }))
                .into_any_element()
        })
    }

    fn stash_drop_confirmation_view(
        &self,
        colors: &ui_kit::ThemeColors,
        cx: &mut Context<Self>,
    ) -> Option<gpui::AnyElement> {
        (self.pending_stash_action == Some(StashAction::Drop)).then(|| {
            div()
                .p_2()
                .bg(colors.raised_background)
                .border_1()
                .border_color(colors.border)
                .child("Drop the latest stash permanently? This cannot be undone.")
                .child(file_action_button(
                    "Confirm drop latest stash",
                    colors,
                    cx,
                    GitronimoApp::drop_latest_stash,
                ))
                .into_any_element()
        })
    }

    fn branch_delete_confirmation_view(
        &self,
        colors: &ui_kit::ThemeColors,
        cx: &mut Context<Self>,
    ) -> Option<gpui::AnyElement> {
        self.pending_branch_delete.as_ref().map(|branch| {
            div()
                .p_2()
                .bg(colors.raised_background)
                .border_1()
                .border_color(colors.border)
                .child(format!(
                    "Delete local branch {branch}? Safe deletion refuses unmerged work."
                ))
                .child(file_action_button(
                    "Delete merged branch",
                    colors,
                    cx,
                    |app, cx| {
                        app.confirm_branch_delete(false, cx);
                    },
                ))
                .child(file_action_button(
                    "Force delete unmerged branch",
                    colors,
                    cx,
                    |app, cx| app.confirm_branch_delete(true, cx),
                ))
                .into_any_element()
        })
    }

    fn network_cancel_button(
        &self,
        colors: &ui_kit::ThemeColors,
        cx: &mut Context<Self>,
    ) -> Option<gpui::AnyElement> {
        self.network_operation.as_ref().map(|_| {
            file_action_button("Cancel network operation", colors, cx, |app, cx| {
                app.cancel_network_operation(cx);
            })
            .into_any_element()
        })
    }

    fn force_with_lease_confirmation_view(
        &self,
        colors: &ui_kit::ThemeColors,
        cx: &mut Context<Self>,
    ) -> Option<gpui::AnyElement> {
        (self.force_push_state == ForcePushState::AwaitingConfirmation).then(|| {
            div()
                .p_2()
                .bg(colors.raised_background)
                .border_1()
                .border_color(colors.border)
                .child("Force-with-lease can replace remote commits only when your fetched remote ref is current.")
                .child(file_action_button("Confirm force-with-lease", colors, cx, |app, cx| {
                    app.confirm_force_with_lease(cx);
                }))
                .into_any_element()
        })
    }

    fn commit_composer_view(
        &self,
        colors: &ui_kit::ThemeColors,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let enabled = !self.mutation_in_flight
            && !self.commit_subject.trim().is_empty()
            && !self.status_groups().staged.is_empty();
        workspace_section(
            "Commit",
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(format!(
                    "Subject: {}",
                    if self.commit_subject.is_empty() {
                        "(required)"
                    } else {
                        &self.commit_subject
                    }
                ))
                .child(file_action_button("Edit subject", colors, cx, |app, cx| {
                    app.edit_commit_subject(cx);
                }))
                .child(format!(
                    "Body: {}",
                    if self.commit_body.is_empty() {
                        "(optional)"
                    } else {
                        &self.commit_body
                    }
                ))
                .child(file_action_button("Edit body", colors, cx, |app, cx| {
                    app.edit_commit_body(cx);
                }))
                .child(format!(
                    "Amend: {}",
                    if self.commit_amend { "on" } else { "off" }
                ))
                .child(file_action_button("Toggle amend", colors, cx, |app, cx| {
                    app.toggle_commit_amend(cx);
                }))
                .child(format!(
                    "Sign-off: {}",
                    if self.commit_sign_off { "on" } else { "off" }
                ))
                .child(file_action_button(
                    "Toggle sign-off",
                    colors,
                    cx,
                    GitronimoApp::toggle_commit_sign_off,
                ))
                .child(format!("Author: {}", self.author_identity))
                .child(file_action_button(
                    "Commit staged changes",
                    colors,
                    cx,
                    move |app, cx| {
                        if enabled {
                            app.commit_draft(cx);
                        }
                    },
                )),
            colors,
        )
    }

    fn context_menu_view(
        &self,
        repository: &WorktreeRepository,
        colors: &ui_kit::ThemeColors,
        cx: &mut Context<Self>,
    ) -> Option<gpui::AnyElement> {
        self.context_path.as_ref().map(|path| {
            let copy_repository = repository.clone();
            let reveal_repository = repository.clone();
            let open_repository = repository.clone();
            div()
                .p_2()
                .bg(colors.raised_background)
                .border_1()
                .border_color(colors.border)
                .child(format!(
                    "File actions: {}",
                    String::from_utf8_lossy(&path.0)
                ))
                .child(
                    div()
                        .flex()
                        .gap_2()
                        .child(file_action_button(
                            "Copy path",
                            colors,
                            cx,
                            move |app, cx| {
                                app.copy_context_path(&copy_repository, cx);
                            },
                        ))
                        .child(file_action_button(
                            "Reveal in Finder",
                            colors,
                            cx,
                            move |app, cx| {
                                app.open_context_path(&reveal_repository, true, cx);
                            },
                        ))
                        .child(file_action_button(
                            "Open in editor",
                            colors,
                            cx,
                            move |app, cx| {
                                app.open_context_path(&open_repository, false, cx);
                            },
                        )),
                )
                .into_any_element()
        })
    }

    fn status_group_view(
        &self,
        title: &'static str,
        entries: &[&StatusEntry],
        staged: bool,
        colors: &ui_kit::ThemeColors,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let rows = if entries.is_empty() {
            div()
                .text_color(colors.text_muted)
                .child(empty_status_message(title))
                .into_any_element()
        } else {
            div()
                .flex()
                .flex_col()
                .gap_1()
                .children(entries.iter().enumerate().map(|(index, entry)| {
                    let path = status_path(entry).clone();
                    let context_path = path.clone();
                    let selected = self.selected_paths.contains(&path);
                    div()
                        .id((title, index))
                        .px_2()
                        .py_1()
                        .bg(if selected {
                            colors.raised_background
                        } else {
                            colors.panel_background
                        })
                        .border_1()
                        .border_color(colors.border)
                        .cursor_pointer()
                        .on_click(cx.listener(move |app, event: &ClickEvent, _, cx| {
                            app.select_status_path(
                                path.clone(),
                                event.modifiers().secondary() || event.modifiers().shift,
                                staged,
                                cx,
                            );
                        }))
                        .on_mouse_down(
                            MouseButton::Right,
                            cx.listener(move |app, _, _, cx| {
                                app.show_status_context_menu(context_path.clone(), cx);
                            }),
                        )
                        .child(status_label(entry))
                }))
                .into_any_element()
        };
        div()
            .p_3()
            .flex()
            .flex_col()
            .gap_2()
            .bg(colors.panel_background)
            .border_1()
            .border_color(colors.border)
            .child(
                div().flex().justify_between().child(title).child(
                    div()
                        .text_color(colors.text_secondary)
                        .child(entries.len().to_string()),
                ),
            )
            .child(rows)
    }

    fn diff_view(
        &self,
        colors: &ui_kit::ThemeColors,
        cx: &mut Context<Self>,
    ) -> Option<gpui::AnyElement> {
        self.loaded_diff.as_ref().map(|loaded| {
            let mut text = String::new();
            let mut hunk_count = 0;
            for file in &loaded.diff.files {
                for hunk in &file.hunks {
                    hunk_count += 1;
                    text.push_str(&String::from_utf8_lossy(&hunk.header));
                    text.push('\n');
                    for line in &hunk.lines {
                        let prefix = match line.kind {
                            git_domain::DiffLineKind::Context => ' ',
                            git_domain::DiffLineKind::Addition => '+',
                            git_domain::DiffLineKind::Removal => '-',
                        };
                        text.push(prefix);
                        text.push_str(&String::from_utf8_lossy(&line.content));
                        text.push('\n');
                    }
                }
            }
            let can_mutate_hunks = !self.mutation_in_flight
                && !loaded.truncated
                && !loaded.diff.files.iter().any(|file| file.binary)
                && self.selected_diff.is_some();
            let staged_diff = matches!(&self.selected_diff, Some((_, true)));
            div()
                .p_3()
                .flex()
                .flex_col()
                .gap_2()
                .bg(colors.panel_background)
                .border_1()
                .border_color(colors.border)
                .children((can_mutate_hunks && hunk_count > 0).then(|| {
                    div()
                        .flex()
                        .gap_2()
                        .children((0..hunk_count).map(|hunk_index| {
                            div()
                                .id(("diff-hunk", hunk_index))
                                .px_2()
                                .py_1()
                                .bg(colors.raised_background)
                                .border_1()
                                .border_color(colors.border)
                                .cursor_pointer()
                                .on_click(cx.listener(move |app, _, _, cx| {
                                    if staged_diff {
                                        app.unstage_diff_hunk(hunk_index, cx);
                                    } else {
                                        app.stage_diff_hunk(hunk_index, cx);
                                    }
                                }))
                                .child(format!(
                                    "{} hunk {}",
                                    if staged_diff { "Unstage" } else { "Stage" },
                                    hunk_index + 1
                                ))
                        }))
                }))
                .child(if loaded.diff.files.iter().any(|file| file.binary) {
                    "Binary file changed".to_owned()
                } else {
                    text
                })
                .children(loaded.truncated.then(|| {
                    div().child("Diff truncated.").child(file_action_button(
                        "Load full diff",
                        colors,
                        cx,
                        |app, cx| {
                            app.load_full_diff(cx);
                        },
                    ))
                }))
                .into_any_element()
        })
    }

    fn welcome_view(
        &self,
        colors: &ui_kit::ThemeColors,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let recent_rows = if self.recents.is_empty() {
            state_panel(
                "No recent repositories",
                "Open a Git repository to keep it here for next time.",
                colors.text_muted,
                colors,
            )
        } else {
            div()
                .flex()
                .flex_col()
                .gap_2()
                .children(self.recents.iter().enumerate().map(|(index, path)| {
                    let path = path.clone();
                    let label = path.display().to_string();
                    div()
                        .id(("recent-repository", index))
                        .p_3()
                        .bg(colors.raised_background)
                        .border_1()
                        .border_color(colors.border)
                        .cursor_pointer()
                        .on_click(cx.listener(move |app, _, window, cx| {
                            app.open_recent(path.clone(), window, cx);
                        }))
                        .child(div().text_lg().child(label))
                        .child(
                            div()
                                .mt_1()
                                .text_sm()
                                .text_color(colors.text_secondary)
                                .child("Open this repository"),
                        )
                }))
                .into_any_element()
        };
        div()
            .max_w(px(880.0))
            .flex()
            .flex_col()
            .gap_5()
            .child(
                div()
                    .p_6()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .bg(colors.panel_background)
                    .border_1()
                    .border_color(colors.border)
                    .child(div().text_2xl().child("Start with a repository"))
                    .child(
                        div().text_color(colors.text_secondary).child(
                            "Gitronimo keeps your local Git workflow in one focused workspace.",
                        ),
                    )
                    .child(
                        div()
                            .mt_2()
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(primary_window_action_button(
                                "Choose repository",
                                colors,
                                cx,
                                |_, window, cx| {
                                    window.dispatch_action(Box::new(OpenRepository), cx);
                                },
                            ))
                            .child(
                                div()
                                    .text_color(colors.text_secondary)
                                    .child("or drop a folder anywhere in this window"),
                            ),
                    ),
            )
            .child(div().flex().flex_col().gap_2().children([
                welcome_feature_card(
                    "Review changes",
                    "Inspect staged, unstaged, and untracked files before committing.",
                    colors,
                ),
                welcome_feature_card(
                    "Trace history",
                    "Browse commits and follow the branch context without leaving the workspace.",
                    colors,
                ),
                welcome_feature_card(
                    "Stay in control",
                    "Use your installed Git, credentials, hooks, and signing configuration.",
                    colors,
                ),
            ]))
            .child(div().mt_3().text_lg().child("Recent repositories"))
            .child(recent_rows)
    }
}

#[derive(Default)]
struct StatusGroups<'a> {
    staged: Vec<&'a StatusEntry>,
    unstaged: Vec<&'a StatusEntry>,
    untracked: Vec<&'a StatusEntry>,
    conflicts: Vec<&'a StatusEntry>,
}

fn status_badge(
    label: &'static str,
    count: usize,
    colors: &ui_kit::ThemeColors,
) -> gpui::AnyElement {
    div()
        .flex()
        .justify_between()
        .text_color(colors.text_secondary)
        .child(label)
        .child(count.to_string())
        .into_any_element()
}

fn welcome_sidebar_view(width: f32, colors: &ui_kit::ThemeColors) -> gpui::AnyElement {
    div()
        .w(px(width))
        .h_full()
        .p_4()
        .flex()
        .flex_col()
        .gap_3()
        .bg(colors.sidebar_background)
        .border_r_1()
        .border_color(colors.border)
        .child(div().text_lg().child("Workspace"))
        .child(
            div()
                .text_color(colors.text_secondary)
                .child("Open a repository to start reviewing changes, history, and remotes."),
        )
        .child(
            div()
                .mt_4()
                .text_color(colors.text_muted)
                .child("Quick start"),
        )
        .child(
            div()
                .p_3()
                .bg(colors.raised_background)
                .border_1()
                .border_color(colors.border)
                .child("Open a local repository")
                .child(
                    div()
                        .mt_1()
                        .text_sm()
                        .text_color(colors.text_secondary)
                        .child("Command-O or drag a folder into this window"),
                ),
        )
        .child(
            div()
                .mt_4()
                .text_color(colors.text_muted)
                .child("Available here"),
        )
        .child("Working copy and file diffs")
        .child("History and local branches")
        .child("Configured remotes")
        .into_any_element()
}

fn workspace_section(
    title: &'static str,
    content: impl IntoElement,
    colors: &ui_kit::ThemeColors,
) -> gpui::AnyElement {
    div()
        .p_3()
        .flex()
        .flex_col()
        .gap_2()
        .bg(colors.panel_background)
        .border_1()
        .border_color(colors.border)
        .child(
            div()
                .text_sm()
                .text_color(colors.text_secondary)
                .child(title),
        )
        .child(content)
        .into_any_element()
}

fn mutation_button(
    label: &'static str,
    disabled: bool,
    operation: Mutation,
    colors: &ui_kit::ThemeColors,
    cx: &mut Context<GitronimoApp>,
) -> gpui::AnyElement {
    let tooltip_colors = *colors;
    div()
        .id(label)
        .px_2()
        .py_1()
        .bg(colors.raised_background)
        .border_1()
        .border_color(colors.border)
        .cursor_pointer()
        .tooltip(move |_, cx| {
            cx.new(|_| ActionTooltip {
                label,
                colors: tooltip_colors,
            })
            .into()
        })
        .on_click(cx.listener(move |app, _, _, cx| {
            if !disabled {
                app.mutate(operation, cx);
            }
        }))
        .child(label)
        .into_any_element()
}

fn file_action_button(
    label: &'static str,
    colors: &ui_kit::ThemeColors,
    cx: &mut Context<GitronimoApp>,
    on_click: impl Fn(&mut GitronimoApp, &mut Context<GitronimoApp>) + 'static,
) -> gpui::AnyElement {
    let tooltip_colors = *colors;
    div()
        .id(label)
        .px_2()
        .py_1()
        .bg(colors.panel_background)
        .border_1()
        .border_color(colors.border)
        .cursor_pointer()
        .tooltip(move |_, cx| {
            cx.new(|_| ActionTooltip {
                label,
                colors: tooltip_colors,
            })
            .into()
        })
        .on_click(cx.listener(move |app, _, _, cx| on_click(app, cx)))
        .child(label)
        .into_any_element()
}

fn window_action_button(
    label: &'static str,
    colors: &ui_kit::ThemeColors,
    cx: &mut Context<GitronimoApp>,
    on_click: impl Fn(&mut GitronimoApp, &mut Window, &mut Context<GitronimoApp>) + 'static,
) -> gpui::AnyElement {
    let tooltip_colors = *colors;
    div()
        .id(label)
        .px_3()
        .py_2()
        .bg(colors.raised_background)
        .border_1()
        .border_color(colors.border)
        .cursor_pointer()
        .tooltip(move |_, cx| {
            cx.new(|_| ActionTooltip {
                label,
                colors: tooltip_colors,
            })
            .into()
        })
        .on_click(cx.listener(move |app, _, window, cx| on_click(app, window, cx)))
        .child(label)
        .into_any_element()
}

fn primary_window_action_button(
    label: &'static str,
    colors: &ui_kit::ThemeColors,
    cx: &mut Context<GitronimoApp>,
    on_click: impl Fn(&mut GitronimoApp, &mut Window, &mut Context<GitronimoApp>) + 'static,
) -> gpui::AnyElement {
    let tooltip_colors = *colors;
    div()
        .id(label)
        .px_3()
        .py_2()
        .bg(colors.accent)
        .border_1()
        .border_color(colors.accent)
        .text_color(colors.panel_background)
        .cursor_pointer()
        .tooltip(move |_, cx| {
            cx.new(|_| ActionTooltip {
                label,
                colors: tooltip_colors,
            })
            .into()
        })
        .on_click(cx.listener(move |app, _, window, cx| on_click(app, window, cx)))
        .child(label)
        .into_any_element()
}

fn welcome_feature_card(
    title: &'static str,
    description: &'static str,
    colors: &ui_kit::ThemeColors,
) -> gpui::AnyElement {
    div()
        .flex_1()
        .p_4()
        .flex()
        .flex_col()
        .gap_2()
        .bg(colors.panel_background)
        .border_1()
        .border_color(colors.border)
        .child(title)
        .child(div().text_color(colors.text_secondary).child(description))
        .into_any_element()
}

struct ActionTooltip {
    label: &'static str,
    colors: ui_kit::ThemeColors,
}

impl Render for ActionTooltip {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .px_2()
            .py_1()
            .bg(self.colors.raised_background)
            .border_1()
            .border_color(self.colors.border)
            .text_color(self.colors.text_primary)
            .child(self.label)
    }
}

fn validated_action_button(
    label: &'static str,
    enabled: bool,
    unavailable_reason: &'static str,
    colors: &ui_kit::ThemeColors,
    cx: &mut Context<GitronimoApp>,
    on_click: impl Fn(&mut GitronimoApp, &mut Context<GitronimoApp>) + 'static,
) -> gpui::AnyElement {
    if enabled {
        return file_action_button(label, colors, cx, on_click);
    }
    let tooltip_colors = *colors;
    div()
        .id(label)
        .px_2()
        .py_1()
        .bg(colors.raised_background)
        .border_1()
        .border_color(colors.border)
        .text_color(colors.text_muted)
        .tooltip(move |_, cx| {
            cx.new(|_| ActionTooltip {
                label: unavailable_reason,
                colors: tooltip_colors,
            })
            .into()
        })
        .child(label)
        .into_any_element()
}

fn status_path(entry: &StatusEntry) -> &GitPath {
    match entry {
        StatusEntry::Ordinary { path, .. }
        | StatusEntry::Renamed { path, .. }
        | StatusEntry::Unmerged { path, .. }
        | StatusEntry::Untracked(path)
        | StatusEntry::Ignored(path) => path,
    }
}

fn status_label(entry: &StatusEntry) -> String {
    let path = String::from_utf8_lossy(&status_path(entry).0);
    match entry {
        StatusEntry::Ordinary { status, .. } => {
            format!("{}  {path}", String::from_utf8_lossy(&status.0))
        }
        StatusEntry::Renamed {
            status,
            source_path,
            ..
        } => format!(
            "{}  {} → {path}",
            String::from_utf8_lossy(&status.0),
            String::from_utf8_lossy(&source_path.0)
        ),
        StatusEntry::Unmerged { .. } => format!("UU  {path}"),
        StatusEntry::Untracked(_) => format!("??  {path}"),
        StatusEntry::Ignored(_) => format!("!!  {path}"),
    }
}

fn loading_view(path: &Path, colors: &ui_kit::ThemeColors) -> impl IntoElement {
    state_panel(
        "Opening repository",
        &format!(
            "Checking {} with Git. This does not block the window.",
            path.display()
        ),
        colors.warning,
        colors,
    )
}

fn error_view(message: &str, colors: &ui_kit::ThemeColors) -> impl IntoElement {
    state_panel(
        "Unable to open repository",
        &format!("{message} Choose a different folder with Command-O."),
        colors.danger,
        colors,
    )
}

fn state_panel(
    title: &str,
    message: &str,
    accent: gpui::Rgba,
    colors: &ui_kit::ThemeColors,
) -> gpui::AnyElement {
    div()
        .p_4()
        .flex()
        .flex_col()
        .gap_2()
        .bg(colors.panel_background)
        .border_1()
        .border_color(colors.border)
        .child(div().text_color(accent).child(title.to_owned()))
        .child(
            div()
                .text_color(colors.text_secondary)
                .child(message.to_owned()),
        )
        .into_any_element()
}

fn empty_status_message(title: &str) -> &'static str {
    match title {
        "Staged" => "No staged files. Select a change, then stage it when it is ready to commit.",
        "Unstaged" => "No unstaged changes.",
        "Untracked" => "No untracked files.",
        "Conflicts" => "No merge conflicts.",
        _ => "Nothing here yet.",
    }
}

fn activity_color(activity: &str, colors: &ui_kit::ThemeColors) -> gpui::Rgba {
    if activity.contains("failed") || activity.contains("Unable") {
        colors.danger
    } else if activity.contains("complete") || activity.contains("refreshed") {
        colors.success
    } else if activity.ends_with('…') || activity.contains("in progress") {
        colors.warning
    } else {
        colors.text_secondary
    }
}

fn activity_label(activity: &str) -> String {
    if activity.ends_with('…') || activity.contains("in progress") {
        format!("● {activity}")
    } else {
        activity.to_owned()
    }
}

fn resize_width(width: f32) -> f32 {
    (width + 20.0).clamp(MINIMUM_PANE_WIDTH, MAXIMUM_PANE_WIDTH)
}

fn shows_inspector(viewport_width: f32, sidebar_width: f32, inspector_width: f32) -> bool {
    viewport_width >= sidebar_width + inspector_width + MINIMUM_CONTENT_WIDTH
}

#[cfg(test)]
mod tests {
    use super::{
        GitPath, GitronimoApp, LastAction, MAXIMUM_PANE_WIDTH, MINIMUM_PANE_WIDTH, ShellState,
        WorktreeRepository, activity_label, crash_report_body, crash_report_path,
        eligible_trash_path, empty_status_message, git_failure_message, keymap,
        network_failure_message, repository_is_available, resize_width, shows_inspector,
        window_options, window_title,
    };
    use app_core::RecentRepositoryStore;
    use gpui::{AppContext, Keystroke, TestAppContext};

    #[gpui::test]
    fn opens_the_welcome_window(cx: &mut TestAppContext) {
        cx.update(|cx| {
            cx.open_window(window_options(cx, None), |window, cx| {
                cx.new(|cx| {
                    GitronimoApp::welcome(
                        Vec::new(),
                        RecentRepositoryStore::new(
                            std::env::temp_dir().join("gitronimo-test-recents.json"),
                        ),
                        window,
                        cx,
                    )
                })
            })
            .expect("the welcome window should open in GPUI's test platform");
        });
    }

    #[gpui::test]
    fn keybindings_dispatch_global_actions(cx: &mut TestAppContext) {
        cx.update(|cx| cx.bind_keys(keymap::bindings()));
        let window = cx.update(|cx| {
            cx.open_window(window_options(cx, None), |window, cx| {
                cx.new(|cx| {
                    GitronimoApp::welcome(
                        Vec::new(),
                        RecentRepositoryStore::new(
                            std::env::temp_dir().join("gitronimo-test-recents.json"),
                        ),
                        window,
                        cx,
                    )
                })
            })
            .expect("the test window should open")
        });
        window
            .update(cx, |app, window, _| window.focus(&app.focus_handle))
            .expect("window should remain open");
        cx.dispatch_keystroke(
            *window,
            Keystroke::parse("cmd-r").expect("valid keybinding"),
        );
        window
            .update(cx, |app, _, _| {
                assert_eq!(app.last_action, Some(LastAction::Refresh));
            })
            .expect("window should remain open");
    }

    #[test]
    fn pane_widths_stay_within_the_safe_range() {
        assert!((resize_width(0.0) - MINIMUM_PANE_WIDTH).abs() < f32::EPSILON);
        assert!((resize_width(MAXIMUM_PANE_WIDTH) - MAXIMUM_PANE_WIDTH).abs() < f32::EPSILON);
    }

    #[test]
    fn inspector_yields_space_to_the_main_content_in_narrow_windows() {
        assert!(!shows_inspector(800.0, 220.0, 320.0));
        assert!(shows_inspector(900.0, 220.0, 320.0));
    }

    #[test]
    fn error_shell_is_explicit() {
        assert!(matches!(
            ShellState::Error("message".into()),
            ShellState::Error(_)
        ));
    }

    #[test]
    fn network_failures_are_actionable_without_echoing_remote_output() {
        assert!(
            network_failure_message("Pushing", "Permission denied (publickey)")
                .contains("authentication was rejected")
        );
        assert!(
            network_failure_message("Pushing", "rejected non-fast-forward")
                .contains("remote has newer commits")
        );
        assert!(
            !network_failure_message("Fetching", "https://token@example.test/repo")
                .contains("token@example.test")
        );
    }

    #[test]
    fn workspace_empty_and_loading_copy_explain_the_next_state() {
        assert!(empty_status_message("Staged").contains("stage"));
        assert_eq!(empty_status_message("Conflicts"), "No merge conflicts.");
        assert_eq!(
            activity_label("Fetching origin in progress. You can cancel it."),
            "● Fetching origin in progress. You can cancel it."
        );
    }

    #[test]
    fn window_titles_distinguish_welcome_loading_and_drafts() {
        assert_eq!(window_title(&ShellState::Welcome, false), "Gitronimo");
        assert_eq!(
            window_title(&ShellState::Loading("/tmp/example".into()), false),
            "Opening repository — Gitronimo"
        );
        assert_eq!(keymap::bindings().len(), 12);
    }

    #[test]
    fn repository_loss_and_index_locks_have_safe_recovery_messages() {
        let root =
            std::env::temp_dir().join(format!("gitronimo-availability-{}", std::process::id()));
        let git_dir = root.join(".git");
        std::fs::create_dir_all(&git_dir).expect("fixture repository should exist");
        let repository = WorktreeRepository {
            worktree_root: root.clone(),
            git_dir,
        };
        assert!(repository_is_available(&repository));
        std::fs::remove_dir_all(&root).expect("fixture repository should remove");
        assert!(!repository_is_available(&repository));
        let message = git_failure_message("Stage selected", "fatal: .git/index.lock: File exists");
        assert!(message.contains("no Git process"));
        assert!(message.contains("before removing it manually"));
    }

    #[test]
    fn crash_reports_are_local_and_do_not_include_panic_payloads() {
        let directory = std::env::temp_dir();
        assert!(
            crash_report_path(&directory, 42)
                .file_name()
                .is_some_and(|name| name == "gitronimo-crash-42.txt")
        );
        let report = crash_report_body(42, Some(std::panic::Location::caller()));
        assert!(report.contains("Timestamp: 42"));
        assert!(report.contains("never uploaded automatically"));
        assert!(!report.contains("secret panic payload"));
    }

    #[test]
    fn trash_refuses_unsafe_paths_symlinks_and_nested_repositories() {
        let root =
            std::env::temp_dir().join(format!("gitronimo-trash-test-{}", std::process::id()));
        std::fs::create_dir_all(&root).expect("temporary root should exist");
        let nested = root.join("nested");
        std::fs::create_dir_all(nested.join(".git"))
            .expect("nested repository marker should exist");
        std::os::unix::fs::symlink(&nested, root.join("link")).expect("symlink should exist");
        assert!(eligible_trash_path(&root, &GitPath(b"../outside".to_vec())).is_err());
        assert!(eligible_trash_path(&root, &GitPath(b"link".to_vec())).is_err());
        assert!(eligible_trash_path(&root, &GitPath(b"nested".to_vec())).is_err());
        std::fs::remove_dir_all(root).expect("temporary root should be removed");
    }
}
