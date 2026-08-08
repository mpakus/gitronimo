//! Commit composer: subject, body, amend, sign-off, author identity, commit action.

use gpui::{div, prelude::*};
use ui_kit::ThemeColors;

use crate::app_state::GitronimoApp;
use crate::views::components::{file_action_button, workspace_section};

impl GitronimoApp {
    pub(crate) fn commit_composer_view(
        &self,
        colors: &ThemeColors,
        cx: &mut gpui::Context<Self>,
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
}
