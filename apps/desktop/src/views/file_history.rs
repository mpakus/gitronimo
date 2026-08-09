//! File history view: a bounded commit list for a single tracked path.

use gpui::{div, prelude::*, px};
use ui_kit::ThemeColors;

use git_domain::WorktreeRepository;

use crate::app_state::{GitronimoApp, RepositoryView, ShellState};
use crate::views::components::{file_action_button, relative_time};

impl GitronimoApp {
    pub(crate) fn file_history_view(
        &self,
        _repository: &WorktreeRepository,
        colors: &ThemeColors,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let rows = self
            .file_history
            .iter()
            .map(|commit| {
                let when = relative_time(commit.author.timestamp);
                (
                    commit.oid.clone(),
                    String::from_utf8_lossy(&commit.author.name).to_string(),
                    when,
                    String::from_utf8_lossy(&commit.subject).to_string(),
                )
            })
            .collect::<Vec<_>>();
        let list_colors = *colors;
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(div().text_xl().child("File history"))
            .child(div().child(format!("Path: {}", self.file_history_path)))
            .child(file_action_button("Working Copy", colors, cx, |app, cx| {
                app.navigate_to(RepositoryView::WorkingCopy, cx);
            }))
            .child(file_action_button(
                "Show history for another path…",
                colors,
                cx,
                |_, cx| GitronimoApp::prompt_file_history(cx),
            ))
            .children(
                rows.into_iter()
                    .enumerate()
                    .map(|(index, (oid, name, when, subject))| {
                        div()
                            .id(index)
                            .h(px(28.0))
                            .px_2()
                            .flex()
                            .items_center()
                            .bg(list_colors.panel_background)
                            .border_b_1()
                            .border_color(list_colors.border)
                            .child(format!("{oid}  {name}  {when}  {subject}"))
                    }),
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
    }
}
