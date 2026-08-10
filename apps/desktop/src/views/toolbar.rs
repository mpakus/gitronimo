//! Top window toolbar: navigation, repository context, command palette, open repository.

use gpui::{AnyElement, div, prelude::*, px};

use crate::actions::{CommandPalette, NavigateBack, NavigateForward, OpenRepository};
use crate::app_state::{GitronimoApp, ShellState};
use crate::views::components::{ActionTooltip, primary_window_action_button, window_action_button};

impl GitronimoApp {
    pub(crate) fn workspace_toolbar(
        &self,
        colors: &ui_kit::ThemeColors,
        cx: &mut gpui::Context<Self>,
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
                    .child(div().text_color(colors.text_secondary).child(context))
                    .children(self.navigation_buttons(colors, cx)),
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

    fn navigation_buttons(
        &self,
        colors: &ui_kit::ThemeColors,
        cx: &mut gpui::Context<Self>,
    ) -> Vec<AnyElement> {
        let has_back = !self.navigation_back.is_empty();
        let has_forward = !self.navigation_forward.is_empty();
        vec![
            navigation_toolbar_button(
                "Back",
                !has_back,
                "No earlier view to return to.",
                colors,
                cx,
                |_, window, cx| {
                    window.dispatch_action(Box::new(NavigateBack), cx);
                },
            ),
            navigation_toolbar_button(
                "Forward",
                !has_forward,
                "No later view to replay.",
                colors,
                cx,
                |_, window, cx| {
                    window.dispatch_action(Box::new(NavigateForward), cx);
                },
            ),
        ]
    }
}

fn navigation_toolbar_button(
    label: &'static str,
    disabled: bool,
    unavailable_reason: &'static str,
    colors: &ui_kit::ThemeColors,
    cx: &mut gpui::Context<GitronimoApp>,
    on_click: impl Fn(&mut GitronimoApp, &mut gpui::Window, &mut gpui::Context<GitronimoApp>) + 'static,
) -> AnyElement {
    let tooltip_colors = *colors;
    div()
        .id(label)
        .px_2()
        .py_1()
        .bg(colors.raised_background)
        .border_1()
        .border_color(colors.border)
        .text_color(if disabled {
            colors.text_muted
        } else {
            colors.text_primary
        })
        .cursor_pointer()
        .tooltip(move |_, cx| {
            cx.new(|_| ActionTooltip {
                label: if disabled { unavailable_reason } else { label },
                colors: tooltip_colors,
            })
            .into()
        })
        .on_click(cx.listener(move |app, _, window, cx| {
            if !disabled {
                on_click(app, window, cx);
            }
        }))
        .child(label)
        .into_any_element()
}
