//! Commit composer: subject, body, amend, sign-off, author identity, commit action.

use gpui::{div, prelude::*, px};
use ui_kit::ThemeColors;

use crate::app_state::GitronimoApp;
use crate::views::components::{file_action_button, primary_window_action_button};

impl GitronimoApp {
    #[allow(clippy::too_many_lines)]
    pub(crate) fn commit_composer_view(
        &self,
        colors: &ThemeColors,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let enabled = !self.mutation_in_flight
            && !self.commit_subject.trim().is_empty()
            && !self.status_groups().staged.is_empty();
        let subject_placeholder = "Summary (required)";
        let body_placeholder = "Description (optional)";
        let subject = if self.commit_subject.is_empty() {
            subject_placeholder.to_owned()
        } else {
            self.commit_subject.clone()
        };
        let body = if self.commit_body.is_empty() {
            body_placeholder.to_owned()
        } else {
            self.commit_body.clone()
        };
        let subject_is_empty = self.commit_subject.is_empty();
        let body_is_empty = self.commit_body.is_empty();
        div()
            .px_2()
            .py_1()
            .flex()
            .flex_col()
            .gap_1()
            .child(
                div()
                    .id("commit-subject-field")
                    .h(px(28.0))
                    .px_3()
                    .flex()
                    .items_center()
                    .bg(colors.panel_background)
                    .rounded(px(4.0))
                    .border_1()
                    .border_color(if subject_is_empty {
                        colors.border
                    } else {
                        colors.accent
                    })
                    .cursor_pointer()
                    .on_click(cx.listener(|app, _, _, cx| {
                        app.edit_commit_subject(cx);
                    }))
                    .text_sm()
                    .text_color(if subject_is_empty {
                        colors.text_muted
                    } else {
                        colors.text_primary
                    })
                    .child(subject),
            )
            .when(!body_is_empty || self.commit_body.is_empty(), |this| {
                this.child(
                    div()
                        .id("commit-body-field")
                        .h(px(48.0))
                        .px_3()
                        .py_1()
                        .flex()
                        .items_start()
                        .bg(colors.panel_background)
                        .rounded(px(4.0))
                        .border_1()
                        .border_color(if body_is_empty {
                            colors.border
                        } else {
                            colors.accent
                        })
                        .text_sm()
                        .text_color(if body_is_empty {
                            colors.text_muted
                        } else {
                            colors.text_secondary
                        })
                        .child(body)
                        .on_click(cx.listener(|app, _, _, cx| {
                            app.edit_commit_body(cx);
                        })),
                )
            })
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(file_action_button("Description", colors, cx, |app, cx| {
                        app.edit_commit_body(cx);
                    }))
                    .child(div().w(px(1.0)).h(px(14.0)).bg(colors.border))
                    .child(file_action_button(
                        if self.commit_amend {
                            "Amend ✓"
                        } else {
                            "Amend"
                        },
                        colors,
                        cx,
                        GitronimoApp::toggle_commit_amend,
                    ))
                    .child(file_action_button(
                        if self.commit_sign_off {
                            "Sign-off ✓"
                        } else {
                            "Sign-off"
                        },
                        colors,
                        cx,
                        GitronimoApp::toggle_commit_sign_off,
                    ))
                    .child(
                        div()
                            .flex_1()
                            .text_xs()
                            .text_color(colors.text_muted)
                            .child(self.author_identity.clone()),
                    )
                    .child(primary_window_action_button(
                        "Commit",
                        colors,
                        cx,
                        move |app, _, cx| {
                            if enabled {
                                app.commit_draft(cx);
                            }
                        },
                    )),
            )
    }
}
