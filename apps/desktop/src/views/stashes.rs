//! Stash browser and safe stash actions (Tower-style core parity).

use gpui::{AnyElement, ClickEvent, SharedString, div, prelude::*, px};
use ui_kit::ThemeColors;

use git_domain::DiffLineKind;

use crate::app_state::{GitronimoApp, StashAction};
use crate::views::components::{
    centered_empty_state, file_action_button, short_calendar_date, two_pane_view, view_panel_header,
};

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
            let when = short_calendar_date(stash.timestamp);
            rows.push(
                div()
                    .id(SharedString::from(format!("stash-{index}")))
                    .h(px(48.0))
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
                        app.select_stash(index, cx);
                    }))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap_2()
                            .child(
                                div()
                                    .flex_1()
                                    .min_w(px(0.0))
                                    .overflow_hidden()
                                    .whitespace_nowrap()
                                    .text_ellipsis()
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
                                    .flex_shrink_0()
                                    .text_xs()
                                    .text_color(if selected == Some(index) {
                                        colors.panel_background
                                    } else {
                                        colors.text_muted
                                    })
                                    .child(when),
                            ),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(if selected == Some(index) {
                                colors.panel_background
                            } else {
                                colors.text_muted
                            })
                            .child(stash.oid.chars().take(8).collect::<String>()),
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
                        "Choose a stash entry to inspect, apply, or branch.",
                        colors,
                    )
                },
                |stash| {
                    let reference = stash.reference.clone();
                    let subject = String::from_utf8_lossy(&stash.subject).into_owned();
                    let when = short_calendar_date(stash.timestamp);
                    let short_oid = stash.oid.chars().take(8).collect::<String>();
                    div()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .p_4()
                        .h_full()
                        .overflow_hidden()
                        .child(
                            div()
                                .text_lg()
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .child(reference),
                        )
                        .child(
                            div()
                                .text_sm()
                                .text_color(colors.text_secondary)
                                .child(subject),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(colors.text_muted)
                                .child(format!("{short_oid} · {when}")),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_wrap()
                                .gap_2()
                                .child(file_action_button(
                                    "Apply…",
                                    colors,
                                    cx,
                                    GitronimoApp::open_stash_apply_dialog_for_selection,
                                ))
                                .child(file_action_button(
                                    "Pop…",
                                    colors,
                                    cx,
                                    |app, cx| {
                                        app.request_stash_action(StashAction::Pop, cx);
                                    },
                                ))
                                .child(file_action_button(
                                    "Drop…",
                                    colors,
                                    cx,
                                    |app, cx| {
                                        app.request_stash_action(StashAction::Drop, cx);
                                    },
                                ))
                                .child(file_action_button(
                                    "Branch…",
                                    colors,
                                    cx,
                                    GitronimoApp::prompt_branch_from_selected_stash,
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
                        .child(self.stash_changeset_detail(colors))
                        .into_any_element()
                },
            );
        let header_actions = div()
            .flex()
            .gap_1()
            .child(file_action_button("Refresh", colors, cx, |app, cx| {
                if let crate::app_state::ShellState::Repository(repository) = &app.state {
                    app.load_stashes(repository.clone(), cx);
                }
            }))
            .child(file_action_button(
                "Create stash…",
                colors,
                cx,
                |app, cx| {
                    app.open_stash_save_dialog(false, Vec::new(), cx);
                },
            ))
            .into_any_element();
        two_pane_view(
            view_panel_header("Stashes", colors, Some(header_actions)),
            list,
            detail,
            colors,
        )
    }

    #[allow(clippy::too_many_lines)]
    fn stash_changeset_detail(&self, colors: &ThemeColors) -> AnyElement {
        let file_count = self
            .selected_stash_diff
            .as_ref()
            .map_or(self.selected_stash_paths.len(), |diff| {
                diff.diff.files.len()
            });
        div()
            .flex()
            .flex_col()
            .gap_2()
            .flex_1()
            .min_h(px(0.0))
            .overflow_hidden()
            .child(
                div()
                    .text_xs()
                    .text_color(colors.text_muted)
                    .child(format!("Showing {file_count} file(s)")),
            )
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h(px(0.0))
                    .overflow_hidden()
                    .gap_2()
                    .child(
                        div()
                            .w(px(220.0))
                            .overflow_hidden()
                            .border_1()
                            .border_color(colors.border)
                            .rounded(px(4.0))
                            .children(
                                if self.selected_stash_diff.is_none()
                                    && self.selected_stash_paths.is_empty()
                                {
                                    vec![
                                        div()
                                            .p_2()
                                            .text_xs()
                                            .text_color(colors.text_muted)
                                            .child("Loading changes…")
                                            .into_any_element(),
                                    ]
                                } else if let Some(loaded) = &self.selected_stash_diff {
                                    loaded
                                        .diff
                                        .files
                                        .iter()
                                        .enumerate()
                                        .map(|(index, file)| {
                                            let path = file
                                                .new_path
                                                .as_ref()
                                                .or(file.old_path.as_ref())
                                                .map_or_else(
                                                    || "(unknown)".into(),
                                                    |path| {
                                                        String::from_utf8_lossy(&path.0)
                                                            .into_owned()
                                                    },
                                                );
                                            div()
                                                .id(("stash-file", index))
                                                .px_2()
                                                .py_1()
                                                .text_xs()
                                                .border_b_1()
                                                .border_color(colors.separator)
                                                .child(path)
                                                .into_any_element()
                                        })
                                        .collect()
                                } else {
                                    self.selected_stash_paths
                                        .iter()
                                        .enumerate()
                                        .map(|(index, path)| {
                                            div()
                                                .id(("stash-path", index))
                                                .px_2()
                                                .py_1()
                                                .text_xs()
                                                .border_b_1()
                                                .border_color(colors.separator)
                                                .child(
                                                    String::from_utf8_lossy(&path.0).into_owned(),
                                                )
                                                .into_any_element()
                                        })
                                        .collect()
                                },
                            ),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.0))
                            .overflow_hidden()
                            .child(self.stash_readonly_diff(colors)),
                    ),
            )
            .into_any_element()
    }

    fn stash_readonly_diff(&self, colors: &ThemeColors) -> AnyElement {
        let Some(loaded) = &self.selected_stash_diff else {
            return div()
                .p_2()
                .text_xs()
                .text_color(colors.text_muted)
                .child("Loading diff…")
                .into_any_element();
        };
        let mut rows: Vec<AnyElement> = Vec::new();
        if loaded.diff.files.is_empty() {
            rows.push(
                div()
                    .text_color(colors.text_muted)
                    .child("No file changes in this stash.")
                    .into_any_element(),
            );
        }
        for file in &loaded.diff.files {
            let path = file
                .new_path
                .as_ref()
                .or(file.old_path.as_ref())
                .map(|path| String::from_utf8_lossy(&path.0).into_owned())
                .unwrap_or_default();
            rows.push(
                div()
                    .font_family("Monaco")
                    .text_color(colors.text_secondary)
                    .child(format!("File: {path}"))
                    .into_any_element(),
            );
            for hunk in &file.hunks {
                rows.push(
                    div()
                        .font_family("Monaco")
                        .text_color(colors.text_secondary)
                        .child(String::from_utf8_lossy(&hunk.header).into_owned())
                        .into_any_element(),
                );
                for line in &hunk.lines {
                    let (sign, color) = match line.kind {
                        DiffLineKind::Addition => ("+", colors.added_line),
                        DiffLineKind::Removal => ("-", colors.removed_line),
                        DiffLineKind::Context => (" ", colors.text_primary),
                    };
                    rows.push(
                        div()
                            .font_family("Monaco")
                            .text_color(color)
                            .child(format!("{sign} {}", String::from_utf8_lossy(&line.content)))
                            .into_any_element(),
                    );
                }
            }
        }
        div()
            .p_2()
            .flex()
            .flex_col()
            .bg(colors.panel_background)
            .border_1()
            .border_color(colors.border)
            .children(rows)
            .into_any_element()
    }
}
