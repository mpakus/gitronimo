//! File history view: a bounded commit list for a single tracked path.

use gpui::{div, prelude::*, px};
use ui_kit::ThemeColors;

use git_domain::WorktreeRepository;

use crate::app_state::{GitronimoApp, RepositoryView, ShellState};
use crate::views::components::{
    centered_empty_state, file_action_button, relative_time, two_pane_view, view_panel_header,
};

impl GitronimoApp {
    #[allow(clippy::too_many_lines)]
    pub(crate) fn file_history_view(
        &self,
        _repository: &WorktreeRepository,
        colors: &ThemeColors,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let list_colors = *colors;
        let mut rows = Vec::new();
        for (index, commit) in self.file_history.iter().enumerate() {
            let when = relative_time(commit.author.timestamp);
            let oid = commit.oid.clone();
            let name = String::from_utf8_lossy(&commit.author.name).to_string();
            let subject = String::from_utf8_lossy(&commit.subject).to_string();
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
                    .bg(list_colors.panel_background)
                    .child(
                        div()
                            .text_sm()
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .text_color(list_colors.text_primary)
                            .child(subject),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(list_colors.text_muted)
                            .child(format!("{oid}  {name}  {when}")),
                    )
                    .into_any_element(),
            );
        }
        let list = if rows.is_empty() {
            centered_empty_state(
                "No file history",
                "Choose a path to inspect its commit history.",
                colors,
            )
        } else {
            div().flex().flex_col().children(rows).into_any_element()
        };
        let detail = if self.file_history_path.is_empty() {
            centered_empty_state(
                "No path selected",
                "Use the command palette to show history for a file.",
                colors,
            )
        } else {
            div()
                .flex()
                .flex_col()
                .gap_3()
                .p_4()
                .child(
                    div()
                        .text_lg()
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .child(self.file_history_path.clone()),
                )
                .child(
                    div()
                        .text_sm()
                        .text_color(colors.text_secondary)
                        .child(format!("{} commit(s) loaded", self.file_history.len())),
                )
                .child(file_action_button(
                    "Blame this path…",
                    colors,
                    cx,
                    |app, cx| {
                        if !app.file_history_path.is_empty() {
                            app.blame_path = app.file_history_path.clone();
                            let ShellState::Repository(repository) = &app.state else {
                                return;
                            };
                            let repository = repository.clone();
                            app.navigate_to(RepositoryView::Blame, cx);
                            app.load_blame(repository, cx);
                            cx.notify();
                        }
                    },
                ))
                .into_any_element()
        };
        let header_actions = div()
            .flex()
            .gap_1()
            .child(file_action_button("Choose path…", colors, cx, |_, cx| {
                GitronimoApp::prompt_file_history(cx);
            }))
            .into_any_element();
        two_pane_view(
            view_panel_header("File History", colors, Some(header_actions)),
            list,
            detail,
            colors,
        )
    }
}
