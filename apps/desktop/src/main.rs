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

use app_core::{RecentRepositoryStore, RepositoryOpenError, WindowGeometry, open_repository};
use git_cli::{CommitRequest, GitExecutable, GitStatusError, read_stderr_limited};
use git_domain::{
    GitPath, GraphState, HeadStatus, HistoryPage, HistoryReference, HistoryRequest, RefSnapshot,
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
    ForcePushState, GitronimoApp, LastAction, Mutation, NetworkOperation, OpenedRepository,
    RefContext, RepositoryView, ShellState, ShortcutReferenceState, ThemeMode,
    appearance_from_window, discard_selected, git_failure_message, network_failure_message,
    repository_is_available, repository_unavailable_message, resize_width,
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
            selected_diff_lines: Vec::new(),
            pending_line_discard: None,
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
            selected_diff_lines: Vec::new(),
            pending_line_discard: None,
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
                self.selected_diff_lines.clear();
                self.pending_line_discard = None;
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
                    app.selected_diff_lines.clear();
                    app.pending_line_discard = None;
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
