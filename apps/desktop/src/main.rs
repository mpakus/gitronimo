//! macOS application entry point.

use std::ops::Range;

mod actions;
mod keymap;
mod menus;

use gpui::{
    App, Application, Bounds, Context, FocusHandle, PathBuilder, Render, Window, WindowBounds,
    WindowOptions, canvas, div, point, prelude::*, px, size, uniform_list,
};

use actions::{OpenRepository, Refresh, ToggleAppearance, WidenInspector, WidenSidebar};
use ui_kit::{Appearance, Theme};

const INITIAL_WINDOW_SIZE: (f32, f32) = (1200.0, 800.0);
const MINIMUM_WINDOW_SIZE: (f32, f32) = (800.0, 560.0);
const SYNTHETIC_COMMIT_COUNT: usize = 100_000;
const MINIMUM_PANE_WIDTH: f32 = 180.0;
const MAXIMUM_PANE_WIDTH: f32 = 440.0;

fn main() {
    Application::new().run(|cx: &mut App| {
        cx.bind_keys(keymap::bindings());
        cx.set_menus(menus::application_menus());

        if let Err(error) = cx.open_window(window_options(cx), |_, cx| cx.new(GitronimoApp::new)) {
            eprintln!("Unable to open the Gitronimo window: {error}");
            return;
        }
        cx.activate(true);
    });
}

fn window_options(cx: &App) -> WindowOptions {
    let initial_size = size(px(INITIAL_WINDOW_SIZE.0), px(INITIAL_WINDOW_SIZE.1));

    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
            None,
            initial_size,
            cx,
        ))),
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
    OpenRepository,
    Refresh,
}

struct GitronimoApp {
    focus_handle: FocusHandle,
    last_action: Option<LastAction>,
    appearance: Appearance,
    sidebar_width: f32,
    inspector_width: f32,
}

impl GitronimoApp {
    fn new(cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            last_action: None,
            appearance: Appearance::Dark,
            sidebar_width: 220.0,
            inspector_width: 320.0,
        }
    }

    fn open_repository(&mut self, _: &OpenRepository, _: &mut Window, cx: &mut Context<Self>) {
        self.last_action = Some(LastAction::OpenRepository);
        cx.notify();
    }

    fn refresh(&mut self, _: &Refresh, _: &mut Window, cx: &mut Context<Self>) {
        self.last_action = Some(LastAction::Refresh);
        cx.notify();
    }

    fn toggle_appearance(&mut self, _: &ToggleAppearance, _: &mut Window, cx: &mut Context<Self>) {
        self.appearance = match self.appearance {
            Appearance::Dark => Appearance::Light,
            Appearance::Light => Appearance::Dark,
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
}

impl Render for GitronimoApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = Theme::for_appearance(self.appearance).colors;
        let sidebar_width = self.sidebar_width;
        let inspector_width = self.inspector_width;

        div()
            .size_full()
            .flex()
            .bg(colors.window_background)
            .text_color(colors.text_primary)
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::open_repository))
            .on_action(cx.listener(Self::refresh))
            .on_action(cx.listener(Self::toggle_appearance))
            .on_action(cx.listener(Self::widen_sidebar))
            .on_action(cx.listener(Self::widen_inspector))
            .child(
                div()
                    .w(px(sidebar_width))
                    .h_full()
                    .p_4()
                    .bg(colors.sidebar_background)
                    .border_r_1()
                    .border_color(colors.border)
                    .child("Workspace"),
            )
            .child(
                div().flex_1().h_full().child(
                    uniform_list(
                        "synthetic-history",
                        SYNTHETIC_COMMIT_COUNT,
                        cx.processor(move |_, range: Range<usize>, _, _| {
                            range
                                .map(|index| {
                                    div()
                                        .id(index)
                                        .h(px(26.0))
                                        .px_3()
                                        .flex()
                                        .items_center()
                                        .border_b_1()
                                        .border_color(colors.separator)
                                        .child(
                                            canvas(
                                                move |bounds, _, _| {
                                                    let x = bounds.origin.x + px(12.0);
                                                    let y = bounds.origin.y;
                                                    let mut path = PathBuilder::stroke(px(2.0));
                                                    path.move_to(point(x, y));
                                                    path.line_to(point(x, y + bounds.size.height));
                                                    path.build().ok()
                                                },
                                                move |_, path, window, _| {
                                                    if let Some(path) = path {
                                                        window.paint_path(
                                                            path,
                                                            colors.graph_lanes
                                                                [index % colors.graph_lanes.len()],
                                                        );
                                                    }
                                                },
                                            )
                                            .w(px(24.0))
                                            .h_full(),
                                        )
                                        .child(format!(
                                            "{:07x}  Synthetic commit {index}",
                                            index * 97
                                        ))
                                })
                                .collect::<Vec<_>>()
                        }),
                    )
                    .size_full(),
                ),
            )
            .child(
                div()
                    .w(px(inspector_width))
                    .h_full()
                    .p_4()
                    .bg(colors.panel_background)
                    .border_l_1()
                    .border_color(colors.border)
                    .child("Inspector"),
            )
    }
}

fn resize_width(width: f32) -> f32 {
    (width + 20.0).clamp(MINIMUM_PANE_WIDTH, MAXIMUM_PANE_WIDTH)
}

#[cfg(test)]
mod tests {
    use super::{
        GitronimoApp, LastAction, MAXIMUM_PANE_WIDTH, MINIMUM_PANE_WIDTH, keymap, resize_width,
        window_options,
    };
    use gpui::{AppContext, Keystroke, TestAppContext};

    #[gpui::test]
    fn opens_the_initial_window(cx: &mut TestAppContext) {
        cx.update(|cx| {
            cx.open_window(window_options(cx), |_, cx| cx.new(GitronimoApp::new))
                .expect("the initial window should open in GPUI's test platform");
        });
    }

    #[gpui::test]
    fn keybindings_dispatch_global_actions(cx: &mut TestAppContext) {
        cx.update(|cx| cx.bind_keys(keymap::bindings()));
        let window = cx.update(|cx| {
            cx.open_window(window_options(cx), |_, cx| cx.new(GitronimoApp::new))
                .expect("the initial window should open in GPUI's test platform")
        });

        window
            .update(cx, |app, window, _| window.focus(&app.focus_handle))
            .expect("the test window should remain open");
        cx.dispatch_keystroke(
            *window,
            Keystroke::parse("cmd-o").expect("valid keybinding"),
        );
        window
            .update(cx, |app, _, _| {
                assert_eq!(app.last_action, Some(LastAction::OpenRepository));
            })
            .expect("the test window should remain open");

        cx.dispatch_keystroke(
            *window,
            Keystroke::parse("cmd-r").expect("valid keybinding"),
        );
        window
            .update(cx, |app, _, _| {
                assert_eq!(app.last_action, Some(LastAction::Refresh));
            })
            .expect("the test window should remain open");
    }

    #[test]
    fn pane_widths_stay_within_the_safe_range() {
        assert!((resize_width(0.0) - MINIMUM_PANE_WIDTH).abs() < f32::EPSILON);
        assert!((resize_width(MAXIMUM_PANE_WIDTH) - MAXIMUM_PANE_WIDTH).abs() < f32::EPSILON);
    }
}
