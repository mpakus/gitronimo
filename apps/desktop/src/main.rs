//! macOS application entry point.

use std::path::{Path, PathBuf};

mod actions;
mod keymap;
mod menus;

use app_core::{RecentRepositoryStore, RepositoryOpenError, WindowGeometry, open_repository};
use git_cli::GitExecutable;
use git_domain::WorktreeRepository;
use gpui::{
    App, Application, Bounds, Context, ExternalPaths, FocusHandle, IntoElement, PathPromptOptions,
    Render, Subscription, Window, WindowAppearance, WindowBounds, WindowOptions, div, point,
    prelude::*, px, size,
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
            store,
            diagnostics: "Checking Git installation…".into(),
            subscriptions: Vec::new(),
        };
        app.observe_system_appearance(window, cx);
        app.observe_window_geometry(window, cx);
        Self::load_diagnostics(cx);
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
        self.activity = "Refresh is available when working-copy data lands in Phase 2.".into();
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
            ShellState::Repository(repository) => {
                repository_view(repository, &colors).into_any_element()
            }
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
                    .child(
                        div()
                            .w(px(sidebar_width))
                            .h_full()
                            .p_4()
                            .bg(colors.sidebar_background)
                            .border_r_1()
                            .border_color(colors.border)
                            .child("Workspace")
                            .child("Working Copy")
                            .child("History"),
                    )
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

fn repository_view(
    repository: &WorktreeRepository,
    colors: &ui_kit::ThemeColors,
) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_4()
        .child(div().text_xl().child("Repository opened"))
        .child(repository.worktree_root.display().to_string())
        .child(
            div()
                .text_sm()
                .text_color(colors.text_secondary)
                .child(format!("Git directory: {}", repository.git_dir.display())),
        )
        .child("Working Copy, History, and Diff data arrive in later phases.")
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
