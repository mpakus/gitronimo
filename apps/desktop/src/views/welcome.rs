//! Welcome view: start card, feature cards, recent repositories.

use gpui::{div, prelude::*, px};
use ui_kit::ThemeColors;

use crate::actions::OpenRepository;
use crate::app_state::GitronimoApp;
use crate::views::components::{primary_window_action_button, state_panel};

impl GitronimoApp {
    pub(crate) fn welcome_view(
        &self,
        colors: &ThemeColors,
        cx: &mut gpui::Context<Self>,
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

fn welcome_feature_card(
    title: &'static str,
    description: &'static str,
    colors: &ThemeColors,
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
