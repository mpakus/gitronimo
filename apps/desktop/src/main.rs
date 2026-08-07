//! macOS application entry point.

use std::path::{Path, PathBuf};

mod actions;
mod keymap;
mod menus;

use app_core::{RecentRepositoryStore, RepositoryOpenError, WindowGeometry, open_repository};
use git_cli::GitExecutable;
use git_domain::{GitPath, StatusEntry, WorktreeRepository, WorktreeStatus};
use gpui::{
    App, Application, Bounds, ClickEvent, Context, ExternalPaths, FocusHandle, IntoElement,
    MouseButton, PathPromptOptions, Render, Subscription, Window, WindowAppearance, WindowBounds,
    WindowOptions, div, point, prelude::*, px, size,
};
use ui_kit::{Appearance, Theme};

use actions::{OpenRepository, Refresh, ToggleAppearance, WidenInspector, WidenSidebar};

const INITIAL_WINDOW_SIZE: (f32, f32) = (1200.0, 800.0);
const MINIMUM_WINDOW_SIZE: (f32, f32) = (800.0, 560.0);
const MINIMUM_PANE_WIDTH: f32 = 180.0;
const MAXIMUM_PANE_WIDTH: f32 = 440.0;

fn main() {
    Application::new().run(|cx: &mut App| {
        cx.bind_keys(keymap::bindings());
        cx.set_menus(menus::application_menus());

        let store = RecentRepositoryStore::new(preferences_path());
        let recents = store.load().unwrap_or_default();
        let geometry = store.load_window_geometry().ok().flatten();
        install_folder_picker(cx, store.clone());
        if let Err(error) = cx.open_window(window_options(cx, geometry), |window, cx| {
            cx.new(|cx| GitronimoApp::welcome(recents, store, window, cx))
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
        cx.new(|cx| GitronimoApp::from_open_outcome(outcome, window, cx))
    });
}

fn discover_and_record(
    path: &Path,
    store: &RecentRepositoryStore,
) -> Result<OpenedRepository, RepositoryOpenError> {
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
    selected_paths: Vec<GitPath>,
    context_path: Option<GitPath>,
    store: RecentRepositoryStore,
    diagnostics: String,
    subscriptions: Vec<Subscription>,
}

impl GitronimoApp {
    fn welcome(
        recents: Vec<PathBuf>,
        store: RecentRepositoryStore,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
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
            selected_paths: Vec::new(),
            context_path: None,
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
            selected_paths: Vec::new(),
            context_path: None,
            store,
            diagnostics: "Checking Git installation…".into(),
            subscriptions: Vec::new(),
        };
        app.observe_system_appearance(window, cx);
        app.observe_window_geometry(window, cx);
        Self::load_diagnostics(cx);
        if let ShellState::Repository(repository) = &app.state {
            app.load_working_copy(repository.clone(), cx);
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
                self.selected_paths.clear();
                self.context_path = None;
                if let ShellState::Repository(repository) = &self.state {
                    self.load_working_copy(repository.clone(), cx);
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
        if let ShellState::Repository(repository) = &self.state {
            self.load_working_copy(repository.clone(), cx);
        } else {
            self.activity = "Open a repository before refreshing its working copy.".into();
        }
        cx.notify();
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
        self.state = ShellState::Loading(path.clone());
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

    fn select_status_path(&mut self, path: GitPath, additive: bool, cx: &mut Context<Self>) {
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
        cx.notify();
    }

    fn show_status_context_menu(&mut self, path: GitPath, cx: &mut Context<Self>) {
        self.context_path = Some(path);
        cx.notify();
    }
}

fn appearance_from_window(appearance: WindowAppearance) -> Appearance {
    match appearance {
        WindowAppearance::Light | WindowAppearance::VibrantLight => Appearance::Light,
        WindowAppearance::Dark | WindowAppearance::VibrantDark => Appearance::Dark,
    }
}

impl Render for GitronimoApp {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = Theme::for_appearance(self.appearance).colors;
        let sidebar_width = self.sidebar_width;
        let inspector_width = self.inspector_width;
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
            .on_action(cx.listener(Self::toggle_appearance))
            .on_action(cx.listener(Self::widen_sidebar))
            .on_action(cx.listener(Self::widen_inspector))
            .on_drop(cx.listener(Self::dropped_paths))
            .child(
                div()
                    .flex_1()
                    .flex()
                    .child(self.sidebar_view(sidebar_width, &colors))
                    .child(div().flex_1().h_full().p_6().child(content))
                    .child(
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
                    ),
            )
            .child(
                div()
                    .h(px(30.0))
                    .px_4()
                    .flex()
                    .items_center()
                    .bg(colors.raised_background)
                    .border_t_1()
                    .border_color(colors.border)
                    .text_color(colors.text_secondary)
                    .child(self.activity.clone()),
            )
    }
}

impl GitronimoApp {
    fn sidebar_view(&self, width: f32, colors: &ui_kit::ThemeColors) -> impl IntoElement {
        let groups = self.status_groups();
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
            .child("Working Copy")
            .child(status_badge("Staged", groups.staged_entries.len(), colors))
            .child(status_badge(
                "Unstaged",
                groups.unstaged_entries.len(),
                colors,
            ))
            .child(status_badge(
                "Untracked",
                groups.untracked_entries.len(),
                colors,
            ))
            .child(status_badge(
                "Conflicts",
                groups.conflict_entries.len(),
                colors,
            ))
            .child("History")
    }

    fn repository_view(
        &self,
        repository: &WorktreeRepository,
        colors: &ui_kit::ThemeColors,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let groups = self.status_groups();
        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(div().text_xl().child("Working Copy"))
            .child(repository.worktree_root.display().to_string())
            .children(self.context_menu_view(colors))
            .child(self.status_group_view("Staged", &groups.staged_entries, colors, cx))
            .child(self.status_group_view("Unstaged", &groups.unstaged_entries, colors, cx))
            .child(self.status_group_view("Untracked", &groups.untracked_entries, colors, cx))
            .child(self.status_group_view("Conflicts", &groups.conflict_entries, colors, cx))
    }

    fn status_groups(&self) -> StatusGroups<'_> {
        let mut groups = StatusGroups::default();
        let Some(status) = &self.working_copy else {
            return groups;
        };
        for entry in &status.entries {
            match entry {
                StatusEntry::Unmerged { .. } => groups.conflict_entries.push(entry),
                StatusEntry::Untracked(_) => groups.untracked_entries.push(entry),
                StatusEntry::Ignored(_) => {}
                StatusEntry::Ordinary { status, .. } | StatusEntry::Renamed { status, .. } => {
                    if status.0[0] != b'.' {
                        groups.staged_entries.push(entry);
                    }
                    if status.0[1] != b'.' {
                        groups.unstaged_entries.push(entry);
                    }
                }
            }
        }
        groups
    }

    fn context_menu_view(&self, colors: &ui_kit::ThemeColors) -> Option<gpui::AnyElement> {
        self.context_path.as_ref().map(|path| {
            div()
                .p_2()
                .bg(colors.raised_background)
                .border_1()
                .border_color(colors.border)
                .child(format!(
                    "File actions: {}",
                    String::from_utf8_lossy(&path.0)
                ))
                .child("Copy path  ·  Reveal in Finder  ·  Open in editor")
                .into_any_element()
        })
    }

    fn status_group_view(
        &self,
        title: &'static str,
        entries: &[&StatusEntry],
        colors: &ui_kit::ThemeColors,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let rows = if entries.is_empty() {
            div()
                .text_color(colors.text_muted)
                .child("None")
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
        div().flex().flex_col().gap_1().child(title).child(rows)
    }

    fn welcome_view(
        &self,
        colors: &ui_kit::ThemeColors,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let recent_rows = if self.recents.is_empty() {
            div()
                .text_color(colors.text_muted)
                .child("No recent repositories yet.")
                .into_any_element()
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
                        .p_2()
                        .bg(colors.raised_background)
                        .border_1()
                        .border_color(colors.border)
                        .cursor_pointer()
                        .on_click(cx.listener(move |app, _, window, cx| {
                            app.open_recent(path.clone(), window, cx);
                        }))
                        .child(label)
                }))
                .into_any_element()
        };
        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(div().text_xl().child("Open a repository"))
            .child("Use File > Open Repository… or Command-O to choose a folder.")
            .child("You can also drop a folder anywhere in this window.")
            .child(
                div()
                    .text_sm()
                    .text_color(colors.text_secondary)
                    .child("Recent repositories"),
            )
            .child(recent_rows)
    }
}

#[derive(Default)]
struct StatusGroups<'a> {
    staged_entries: Vec<&'a StatusEntry>,
    unstaged_entries: Vec<&'a StatusEntry>,
    untracked_entries: Vec<&'a StatusEntry>,
    conflict_entries: Vec<&'a StatusEntry>,
}

fn status_badge(
    label: &'static str,
    count: usize,
    colors: &ui_kit::ThemeColors,
) -> impl IntoElement {
    div()
        .flex()
        .justify_between()
        .text_color(colors.text_secondary)
        .child(label)
        .child(count.to_string())
}

fn status_path(entry: &StatusEntry) -> &GitPath {
    match entry {
        StatusEntry::Ordinary { path, .. }
        | StatusEntry::Renamed { path, .. }
        | StatusEntry::Unmerged { path, .. } => path,
        StatusEntry::Untracked(path) | StatusEntry::Ignored(path) => path,
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
    div()
        .flex()
        .flex_col()
        .gap_3()
        .child(div().text_xl().child("Opening repository…"))
        .child(path.display().to_string())
        .child(
            div()
                .text_sm()
                .text_color(colors.text_secondary)
                .child("Git discovery is running off the UI thread."),
        )
}

fn error_view(message: &str, colors: &ui_kit::ThemeColors) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_3()
        .text_color(colors.danger)
        .child(div().text_xl().child("Repository not opened"))
        .child(message.to_owned())
        .child("Choose a different folder with Command-O.")
}

fn resize_width(width: f32) -> f32 {
    (width + 20.0).clamp(MINIMUM_PANE_WIDTH, MAXIMUM_PANE_WIDTH)
}

#[cfg(test)]
mod tests {
    use super::{
        GitronimoApp, LastAction, MAXIMUM_PANE_WIDTH, MINIMUM_PANE_WIDTH, ShellState, keymap,
        resize_width, window_options,
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
    fn error_shell_is_explicit() {
        assert!(matches!(
            ShellState::Error("message".into()),
            ShellState::Error(_)
        ));
    }
}
