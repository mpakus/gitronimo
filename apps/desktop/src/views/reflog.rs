//! Reflog view: a bounded list of HEAD reflog entries with a restore action
//! for recovering deleted branches.

use gpui::{ClickEvent, div, prelude::*, px};
use ui_kit::ThemeColors;

use git_domain::WorktreeRepository;

use crate::app_state::{GitronimoApp, ShellState};
use crate::views::components::{
    centered_empty_state, file_action_button, relative_time, two_pane_view, view_panel_header,
};

impl GitronimoApp {
    #[allow(clippy::too_many_lines)]
    pub(crate) fn reflog_view(
        &self,
        _repository: &WorktreeRepository,
        colors: &ThemeColors,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let selected = self.selected_reflog;
        let list_colors = *colors;
        let mut rows = Vec::new();
        for (index, entry) in self.reflog.iter().enumerate() {
            let when = relative_time(entry.identity.timestamp);
            let oid = String::from_utf8_lossy(&entry.new_oid).to_string();
            let name = String::from_utf8_lossy(&entry.identity.name).to_string();
            let subject = entry.subject.clone();
            rows.push(
                div()
                    .id(index)
                    .h(px(44.0))
                    .px_3()
                    .flex()
                    .flex_col()
                    .justify_center()
                    .gap_0p5()
                    .border_b_1()
                    .border_color(list_colors.separator)
                    .bg(if selected == Some(index) {
                        list_colors.accent
                    } else {
                        list_colors.panel_background
                    })
                    .cursor_pointer()
                    .on_click(cx.listener(move |app, _: &ClickEvent, _, cx| {
                        app.selected_reflog = Some(index);
                        cx.notify();
                    }))
                    .child(
                        div()
                            .text_sm()
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .text_color(if selected == Some(index) {
                                list_colors.panel_background
                            } else {
                                list_colors.text_primary
                            })
                            .child(format!("{oid}  {subject}")),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(if selected == Some(index) {
                                list_colors.panel_background
                            } else {
                                list_colors.text_muted
                            })
                            .child(format!("{name}  {when}")),
                    )
                    .into_any_element(),
            );
        }
        let list = if rows.is_empty() {
            centered_empty_state(
                "No reflog entries",
                "Refresh to load recent HEAD movements.",
                colors,
            )
        } else {
            div().flex().flex_col().children(rows).into_any_element()
        };
        let detail = selected
            .and_then(|index| self.reflog.get(index))
            .map_or_else(
                || {
                    centered_empty_state(
                        "No entry selected",
                        "Choose a reflog entry to restore a branch.",
                        colors,
                    )
                },
                |entry| {
                    let oid = String::from_utf8_lossy(&entry.new_oid).to_string();
                    let name = String::from_utf8_lossy(&entry.identity.name).to_string();
                    let when = relative_time(entry.identity.timestamp);
                    div()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .p_4()
                        .child(
                            div()
                                .text_lg()
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .child(entry.subject.clone()),
                        )
                        .child(div().text_sm().text_color(colors.text_secondary).child(oid))
                        .child(
                            div()
                                .text_xs()
                                .text_color(colors.text_muted)
                                .child(format!("{name}  {when}")),
                        )
                        .child(file_action_button(
                            "Restore branch from selected entry…",
                            colors,
                            cx,
                            |_, cx| GitronimoApp::prompt_restore_branch_from_reflog(cx),
                        ))
                        .into_any_element()
                },
            );
        let header_actions = div()
            .flex()
            .gap_1()
            .child(file_action_button("Refresh", colors, cx, |app, cx| {
                app.reflog_load_token = app.reflog_load_token.wrapping_add(1);
                if let ShellState::Repository(repository) = &app.state {
                    app.load_reflog(repository.clone(), cx);
                }
            }))
            .into_any_element();
        two_pane_view(
            view_panel_header("Reflog", colors, Some(header_actions)),
            list,
            detail,
            colors,
        )
    }
}
