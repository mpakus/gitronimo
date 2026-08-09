//! Blame view: source lines with the commit that introduced each line.

use gpui::{div, prelude::*, px};
use ui_kit::ThemeColors;

use git_domain::WorktreeRepository;

use crate::app_state::{GitronimoApp, RepositoryView, ShellState};
use crate::views::components::{file_action_button, relative_time};

impl GitronimoApp {
    pub(crate) fn blame_view(
        &self,
        _repository: &WorktreeRepository,
        colors: &ThemeColors,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let list_colors = *colors;
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(div().text_xl().child("Blame"))
            .child(div().child(format!("Path: {}", self.blame_path)))
            .child(file_action_button("Working Copy", colors, cx, |app, cx| {
                app.navigate_to(RepositoryView::WorkingCopy, cx);
            }))
            .child(file_action_button(
                "Blame another path…",
                colors,
                cx,
                |_, cx| GitronimoApp::prompt_blame(cx),
            ))
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
            .children(self.blame.iter().enumerate().map(|(index, line)| {
                let oid = String::from_utf8_lossy(&line.oid).to_string();
                let author = String::from_utf8_lossy(&line.author.name).to_string();
                let when = relative_time(line.author.timestamp);
                let content = String::from_utf8_lossy(&line.content).to_string();
                div()
                    .id(index)
                    .h(px(24.0))
                    .px_2()
                    .flex()
                    .items_center()
                    .bg(if index % 2 == 0 {
                        list_colors.panel_background
                    } else {
                        list_colors.raised_background
                    })
                    .border_b_1()
                    .border_color(list_colors.border)
                    .child(format!("{oid}  {author}  {when}  {content}"))
            }))
    }
}
