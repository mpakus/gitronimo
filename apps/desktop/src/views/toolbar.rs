//! Top window toolbar: repository context, command palette, open repository.

use gpui::{div, prelude::*, px};

use crate::actions::{CommandPalette, OpenRepository};
use crate::app_state::{GitronimoApp, ShellState};
use crate::views::components::{primary_window_action_button, window_action_button};

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
}
