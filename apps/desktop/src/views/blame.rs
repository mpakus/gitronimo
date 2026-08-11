//! Blame view: source lines with the commit that introduced each line.

use gpui::{div, prelude::*, px};
use ui_kit::ThemeColors;

use git_domain::WorktreeRepository;

use crate::app_state::{GitronimoApp, RepositoryView, ShellState};
use crate::views::components::{
    centered_empty_state, file_action_button, relative_time, two_pane_view, view_panel_header,
};

impl GitronimoApp {
    #[allow(clippy::too_many_lines)]
    pub(crate) fn blame_view(
        &self,
        _repository: &WorktreeRepository,
        colors: &ThemeColors,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let list_colors = *colors;
        let mut rows = Vec::new();
        for (index, line) in self.blame.iter().enumerate() {
            let oid = String::from_utf8_lossy(&line.oid).to_string();
            let author = String::from_utf8_lossy(&line.author.name).to_string();
            let when = relative_time(line.author.timestamp);
            let content = String::from_utf8_lossy(&line.content).to_string();
            rows.push(
                div()
                    .id(index)
                    .h(px(24.0))
                    .px_3()
                    .flex()
                    .items_center()
                    .gap_2()
                    .border_b_1()
                    .border_color(list_colors.separator)
                    .bg(if index % 2 == 0 {
                        list_colors.panel_background
                    } else {
                        list_colors.raised_background
                    })
                    .child(
                        div()
                            .w(px(72.0))
                            .text_xs()
                            .text_color(list_colors.text_muted)
                            .child(oid),
                    )
                    .child(
                        div()
                            .w(px(120.0))
                            .text_xs()
                            .text_color(list_colors.text_secondary)
                            .child(format!("{author}  {when}")),
                    )
                    .child(
                        div()
                            .flex_1()
                            .font_family("Monaco")
                            .text_xs()
                            .child(content),
                    )
                    .into_any_element(),
            );
        }
        let list = if rows.is_empty() {
            centered_empty_state(
                "No blame data",
                "Choose a path to inspect line-by-line authorship.",
                colors,
            )
        } else {
            div().flex().flex_col().children(rows).into_any_element()
        };
        let detail = if self.blame_path.is_empty() {
            centered_empty_state(
                "No path selected",
                "Use the command palette to blame a file.",
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
                        .child(self.blame_path.clone()),
                )
                .child(
                    div()
                        .text_sm()
                        .text_color(colors.text_secondary)
                        .child(format!("{} line(s) loaded", self.blame.len())),
                )
                .child(file_action_button(
                    "File history for this path…",
                    colors,
                    cx,
                    |app, cx| {
                        if !app.blame_path.is_empty() {
                            app.file_history_path = app.blame_path.clone();
                            let ShellState::Repository(repository) = &app.state else {
                                return;
                            };
                            let repository = repository.clone();
                            app.navigate_to(RepositoryView::FileHistory, cx);
                            app.load_file_history(repository, cx);
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
                GitronimoApp::prompt_blame(cx);
            }))
            .into_any_element();
        two_pane_view(
            view_panel_header("Blame", colors, Some(header_actions)),
            list,
            detail,
            colors,
        )
    }
}
