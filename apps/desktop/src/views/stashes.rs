//! Stash browser and safe stash actions.

use gpui::{ClickEvent, SharedString, div, prelude::*};
use ui_kit::ThemeColors;

use crate::app_state::{GitronimoApp, RepositoryView, StashAction};
use crate::views::components::file_action_button;

impl GitronimoApp {
    #[allow(clippy::too_many_lines)]
    pub(crate) fn stashes_view(
        &self,
        colors: &ThemeColors,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let selected = self.selected_stash;
        let mut rows = Vec::new();
        for (index, stash) in self.stashes.iter().enumerate() {
            let reference = stash.reference.clone();
            let subject = String::from_utf8_lossy(&stash.subject).into_owned();
            let oid = stash.oid.clone();
            rows.push(
                div()
                    .id(SharedString::from(format!("stash-{index}")))
                    .p_2()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .bg(if selected == Some(index) {
                        colors.selection
                    } else {
                        colors.panel_background
                    })
                    .border_1()
                    .border_color(colors.border)
                    .cursor_pointer()
                    .on_click(cx.listener(move |app, _: &ClickEvent, _, cx| {
                        app.selected_stash = Some(index);
                        cx.notify();
                    }))
                    .child(div().text_sm().child(format!("{reference}  {subject}")))
                    .child(div().text_xs().text_color(colors.text_muted).child(oid))
                    .into_any_element(),
            );
        }
        let selected_actions = self.selected_stash.map(|_| {
            div()
                .flex()
                .gap_2()
                .child(file_action_button(
                    "Apply selected",
                    colors,
                    cx,
                    GitronimoApp::apply_stash_by_selection,
                ))
                .child(file_action_button(
                    "Pop selected…",
                    colors,
                    cx,
                    |app, cx| {
                        app.request_stash_action(StashAction::Pop, cx);
                    },
                ))
                .child(file_action_button(
                    "Drop selected…",
                    colors,
                    cx,
                    |app, cx| {
                        app.request_stash_action(StashAction::Drop, cx);
                    },
                ))
        });
        let confirmation =
            self.pending_stash_action_ref
                .as_ref()
                .map(|(action, reference, subject)| {
                    let action_label = match action {
                        StashAction::Pop => "Pop",
                        StashAction::Drop => "Drop",
                    };
                    div()
                        .p_3()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .bg(colors.raised_background)
                        .border_1()
                        .border_color(colors.danger)
                        .child(format!("{action_label} {reference}?"))
                        .child(subject.clone())
                        .child("This action changes or removes the selected stash recovery entry.")
                        .child(
                            div()
                                .flex()
                                .gap_2()
                                .child(file_action_button(
                                    "Confirm",
                                    colors,
                                    cx,
                                    GitronimoApp::confirm_stash_action_ref,
                                ))
                                .child(file_action_button(
                                    "Cancel",
                                    colors,
                                    cx,
                                    GitronimoApp::cancel_stash_action_ref,
                                )),
                        )
                });
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(div().text_xl().child("Stashes"))
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(file_action_button("Working Copy", colors, cx, |app, cx| {
                        app.navigate_to(RepositoryView::WorkingCopy, cx);
                    }))
                    .child(file_action_button("Refresh", colors, cx, |app, cx| {
                        if let crate::app_state::ShellState::Repository(repository) = &app.state {
                            app.load_stashes(repository.clone(), cx);
                        }
                    }))
                    .child(file_action_button("Create stash", colors, cx, |app, cx| {
                        app.create_stash(false, cx);
                    }))
                    .child(file_action_button(
                        "Create with untracked",
                        colors,
                        cx,
                        |app, cx| {
                            app.create_stash(true, cx);
                        },
                    )),
            )
            .children(selected_actions)
            .children(confirmation)
            .children(if self.stashes.is_empty() {
                Some(
                    div()
                        .text_color(colors.text_muted)
                        .child("No stash entries.")
                        .into_any_element(),
                )
            } else {
                None
            })
            .children(rows)
    }
}
