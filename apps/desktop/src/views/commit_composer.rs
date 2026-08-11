//! Commit composer: subject, body, amend, sign-off, author identity, commit action.

use gpui::{div, prelude::*, px};
use ui_kit::ThemeColors;

use crate::app_state::{GitronimoApp, Mutation};
use crate::views::components::{commit_option_chip, mutation_button, primary_window_action_button};
use crate::views::single_line_input::single_line_input_shell;

impl GitronimoApp {
    #[allow(clippy::too_many_lines)]
    pub(crate) fn commit_composer_view(
        &self,
        colors: &ThemeColors,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let staged_count = self.status_groups().staged.len();
        let enabled =
            !self.mutation_in_flight && !self.commit_subject.trim().is_empty() && staged_count > 0;
        let subject_remaining = 50usize.saturating_sub(self.commit_subject.chars().count());
        let groups = self.status_groups();
        let has_stageable = !groups.unstaged.is_empty()
            || !groups.untracked.is_empty()
            || !groups.conflicts.is_empty();
        div()
            .px_2()
            .py_1p5()
            .flex()
            .flex_col()
            .gap_1()
            .border_b_1()
            .border_color(colors.border)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(div().id("commit-subject-field").flex_1().child(
                        single_line_input_shell(self.commit_subject_input.clone(), colors, false),
                    ))
                    .child(
                        div()
                            .text_xs()
                            .text_color(colors.text_muted)
                            .child(subject_remaining.to_string()),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(mutation_button(
                        "Stage All",
                        self.mutation_in_flight || !has_stageable,
                        Mutation::StageAll,
                        colors,
                        cx,
                    ))
                    .child(div().flex_1())
                    .child(div().text_xs().text_color(colors.text_muted).child(
                        if staged_count > 0 {
                            format!("{staged_count} staged")
                        } else {
                            self.author_identity.clone()
                        },
                    ))
                    .child(primary_window_action_button(
                        "Commit",
                        enabled,
                        colors,
                        cx,
                        move |app, _, cx| {
                            app.commit_draft(cx);
                        },
                    )),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_0p5()
                    .child(commit_option_chip(
                        if self.commit_options_expanded {
                            "Description ✓"
                        } else {
                            "Description"
                        },
                        self.commit_options_expanded,
                        colors,
                        cx,
                        |app, cx| {
                            app.commit_options_expanded = !app.commit_options_expanded;
                            cx.notify();
                        },
                    ))
                    .child(commit_option_chip(
                        if self.commit_amend {
                            "Amend ✓"
                        } else {
                            "Amend"
                        },
                        self.commit_amend,
                        colors,
                        cx,
                        GitronimoApp::toggle_commit_amend,
                    ))
                    .child(commit_option_chip(
                        if self.commit_sign_off {
                            "Sign-off ✓"
                        } else {
                            "Sign-off"
                        },
                        self.commit_sign_off,
                        colors,
                        cx,
                        GitronimoApp::toggle_commit_sign_off,
                    )),
            )
            .when(self.commit_options_expanded, |this| {
                this.child(
                    div()
                        .id("commit-body-field")
                        .min_h(px(40.0))
                        .max_h(px(72.0))
                        .child(single_line_input_shell(
                            self.commit_body_input.clone(),
                            colors,
                            false,
                        )),
                )
            })
    }
}
