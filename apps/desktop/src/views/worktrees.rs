//! Worktrees view: list the repository's linked worktrees, add a new one, or
//! remove one.

use gpui::{SharedString, div, prelude::*, px};
use ui_kit::ThemeColors;

use git_domain::WorktreeRepository;

use crate::app_state::{GitronimoApp, ShellState};
use crate::views::components::{
    centered_empty_state, file_action_button, two_pane_view, view_panel_header,
};

impl GitronimoApp {
    pub(crate) fn worktrees_view(
        &self,
        _repository: &WorktreeRepository,
        colors: &ThemeColors,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let list_colors = *colors;
        let mut rows = Vec::new();
        for entry in &self.worktrees {
            let path = String::from_utf8_lossy(&entry.path.0).to_string();
            let head = String::from_utf8_lossy(&entry.head).to_string();
            let branch = entry.branch.as_ref().map_or_else(
                || "detached".to_owned(),
                |branch| String::from_utf8_lossy(&branch.0).into_owned(),
            );
            let marker = if entry.main { "main" } else { "linked" };
            let state = if entry.dirty { "dirty" } else { "clean" };
            rows.push(
                div()
                    .id(SharedString::from(format!("worktree-{marker}-{path}")))
                    .h(px(44.0))
                    .px_3()
                    .flex()
                    .flex_col()
                    .justify_center()
                    .gap_0p5()
                    .border_b_1()
                    .border_color(list_colors.separator)
                    .bg(list_colors.panel_background)
                    .child(div().text_sm().child(format!("{marker}  {path}")))
                    .child(
                        div()
                            .text_xs()
                            .text_color(list_colors.text_muted)
                            .child(format!("{branch}  {head}  {state}")),
                    )
                    .into_any_element(),
            );
        }
        let list = if rows.is_empty() {
            centered_empty_state(
                "No linked worktrees",
                "Add a worktree to work on another branch in parallel.",
                colors,
            )
        } else {
            div().flex().flex_col().children(rows).into_any_element()
        };
        let detail = div()
            .flex()
            .flex_col()
            .gap_3()
            .p_4()
            .child(
                div()
                    .text_sm()
                    .text_color(colors.text_secondary)
                    .child(format!("{} worktree(s) linked", self.worktrees.len())),
            )
            .child(file_action_button(
                "Add worktree…",
                colors,
                cx,
                |_, cx| {
                    GitronimoApp::prompt_add_worktree(cx);
                },
            ))
            .child(file_action_button(
                "Remove worktree…",
                colors,
                cx,
                |_, cx| {
                    GitronimoApp::prompt_remove_worktree(cx);
                },
            ))
            .into_any_element();
        let header_actions = div()
            .flex()
            .gap_1()
            .child(file_action_button("Refresh", colors, cx, |app, cx| {
                app.worktrees_load_token = app.worktrees_load_token.wrapping_add(1);
                if let ShellState::Repository(repository) = &app.state {
                    app.load_worktrees(repository.clone(), cx);
                }
            }))
            .into_any_element();
        two_pane_view(
            view_panel_header("Worktrees", colors, Some(header_actions)),
            list,
            detail,
            colors,
        )
    }
}
