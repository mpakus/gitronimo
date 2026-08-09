//! Worktrees view: list the repository's linked worktrees, add a new one, or
//! remove one.

use gpui::{SharedString, div, prelude::*};
use ui_kit::ThemeColors;

use git_domain::WorktreeRepository;

use crate::app_state::{GitronimoApp, RepositoryView, ShellState};
use crate::views::components::file_action_button;

impl GitronimoApp {
    pub(crate) fn worktrees_view(
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
            .child(div().text_xl().child("Worktrees"))
            .child(file_action_button("Working Copy", colors, cx, |app, cx| {
                app.navigate_to(RepositoryView::WorkingCopy, cx);
            }))
            .child(file_action_button(
                "Add worktree…",
                colors,
                cx,
                |_, cx| GitronimoApp::prompt_add_worktree(cx),
            ))
            .child(file_action_button(
                "Remove worktree…",
                colors,
                cx,
                |_, cx| GitronimoApp::prompt_remove_worktree(cx),
            ))
            .child(file_action_button(
                "Refresh worktrees",
                colors,
                cx,
                |app, cx| {
                    app.worktrees_load_token = app.worktrees_load_token.wrapping_add(1);
                    if let ShellState::Repository(repository) = &app.state {
                        app.load_worktrees(repository.clone(), cx);
                    }
                },
            ))
            .children(self.worktrees.iter().map(|entry| {
                let path = String::from_utf8_lossy(&entry.path.0).to_string();
                let head = String::from_utf8_lossy(&entry.head).to_string();
                let branch = entry.branch.as_ref().map_or_else(
                    || "detached".to_owned(),
                    |branch| String::from_utf8_lossy(&branch.0).into_owned(),
                );
                let marker = if entry.main { "main" } else { "linked" };
                let state = if entry.dirty { "dirty" } else { "clean" };
                div()
                    .id(SharedString::from(format!("worktree-{marker}-{path}")))
                    .px_2()
                    .py_1()
                    .flex()
                    .flex_col()
                    .bg(list_colors.panel_background)
                    .border_b_1()
                    .border_color(list_colors.border)
                    .child(format!("{marker}  {path}"))
                    .child(format!("{branch}  {head}  {state}"))
            }))
    }
}
