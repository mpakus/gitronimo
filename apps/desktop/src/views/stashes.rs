//! Stash browser and safe stash actions.

use gpui::{ClickEvent, SharedString, div, prelude::*, px};
use ui_kit::ThemeColors;

use crate::app_state::{GitronimoApp, StashAction};
use crate::views::components::{centered_empty_state, file_action_button};

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
                    .h(px(44.0))
                    .px_3()
                    .flex()
                    .flex_col()
                    .justify_center()
                    .gap_0p5()
                    .border_b_1()
                    .border_color(colors.separator)
                    .bg(if selected == Some(index) {
                        colors.accent
                    } else {
                        colors.panel_background
                    })
                    .cursor_pointer()
                    .on_click(cx.listener(move |app, _: &ClickEvent, _, cx| {
                        app.selected_stash = Some(index);
                        cx.notify();
                    }))
                    .child(
                        div()
                            .text_sm()
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .text_color(if selected == Some(index) {
                                colors.panel_background
                            } else {
                                colors.text_primary
                            })
                            .child(format!("{reference}  {subject}")),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(if selected == Some(index) {
                                colors.panel_background
                            } else {
                                colors.text_muted
                            })
                            .child(oid),
                    )
                    .into_any_element(),
            );
        }
        let list = if rows.is_empty() {
            centered_empty_state(
                "No stashes",
                "Save work in progress with Save stash.",
                colors,
            )
        } else {
            div().flex().flex_col().children(rows).into_any_element()
        };
        let detail = selected
            .and_then(|index| self.stashes.get(index))
            .map_or_else(
                || {
                    centered_empty_state(
                        "No stash selected",
                        "Choose a stash entry to apply, pop, or drop.",
                        colors,
                    )
                },
                |stash| {
                    let reference = stash.reference.clone();
                    let subject = String::from_utf8_lossy(&stash.subject).into_owned();
                    div()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .p_4()
                        .child(
                            div()
                                .text_lg()
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .child(reference),
                        )
                        .child(div().text_sm().text_color(colors.text_secondary).child(subject))
                        .child(
                            div()
                                .text_xs()
                                .text_color(colors.text_muted)
                                .child(stash.oid.clone()),
                        )
                        .child(
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
                                )),
                        )
                        .children(self.pending_stash_action_ref.as_ref().map(
                            |(action, reference, subject)| {
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
                                    .child(
                                        "This action changes or removes the selected stash recovery entry.",
                                    )
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
                            },
                        ))
                        .into_any_element()
                },
            );
        div()
            .flex()
            .flex_col()
            .h_full()
            .child(
                div()
                    .h(px(36.0))
                    .px_3()
                    .flex()
                    .items_center()
                    .justify_between()
                    .border_b_1()
                    .border_color(colors.border)
                    .child(
                        div()
                            .text_sm()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child("Stashes"),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_1()
                            .child(file_action_button("Refresh", colors, cx, |app, cx| {
                                if let crate::app_state::ShellState::Repository(repository) =
                                    &app.state
                                {
                                    app.load_stashes(repository.clone(), cx);
                                }
                            }))
                            .child(file_action_button("Create stash", colors, cx, |app, cx| {
                                app.create_stash(false, cx);
                            })),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .flex()
                    .items_start()
                    .child(
                        div()
                            .w(px(280.0))
                            .h_full()
                            .border_r_1()
                            .border_color(colors.border)
                            .overflow_hidden()
                            .child(list),
                    )
                    .child(div().flex_1().h_full().overflow_hidden().child(detail)),
            )
    }
}
