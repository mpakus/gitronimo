//! Rebase view: inspect and edit the todo of an interactive rebase in
//! progress, then save the plan and continue.

use gpui::{SharedString, div, prelude::*};
use ui_kit::ThemeColors;

use git_cli::GitExecutable;
use git_domain::{InProgressOperation, WorktreeRepository};

use crate::app_state::{GitronimoApp, RepositoryView, ShellState};
use crate::views::components::file_action_button;

impl GitronimoApp {
    #[allow(clippy::too_many_lines)]
    pub(crate) fn rebase_view(
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
            .child(div().text_xl().child("Interactive rebase"))
            .child(file_action_button("Working Copy", colors, cx, |app, cx| {
                app.navigate_to(RepositoryView::WorkingCopy, cx);
            }))
            .child(file_action_button(
                "Start rebase…",
                colors,
                cx,
                |_, cx| GitronimoApp::prompt_start_rebase(cx),
            ))
            .child(file_action_button("Save plan", colors, cx, |_, cx| {
                GitronimoApp::save_rebase_plan(cx);
            }))
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(file_action_button("Continue", colors, cx, |app, cx| {
                        app.run_worktree_mutation(
                            "Continue rebase".to_owned(),
                            |git, repository| {
                                git.continue_operation(repository, &InProgressOperation::Rebase)
                            },
                            cx,
                        );
                    }))
                    .child(file_action_button("Abort", colors, cx, |app, cx| {
                        app.run_worktree_mutation(
                            "Abort rebase".to_owned(),
                            GitExecutable::rebase_abort,
                            cx,
                        );
                    }))
                    .child(file_action_button("Skip", colors, cx, |app, cx| {
                        app.run_worktree_mutation(
                            "Skip rebase patch".to_owned(),
                            GitExecutable::rebase_skip,
                            cx,
                        );
                    })),
            )
            .child(file_action_button("Refresh plan", colors, cx, |app, cx| {
                let ShellState::Repository(repository) = &app.state else {
                    return;
                };
                let repository = repository.clone();
                app.reload_rebase_plan(&repository, cx);
            }))
            .children(self.rebase_plan.iter().enumerate().map(|(index, item)| {
                let verb = item.action.verb().to_owned();
                let next_verb = item.action.next().verb().to_owned();
                let arguments = item.arguments.clone();
                let can_up = index > 0;
                let can_down = index + 1 < self.rebase_plan.len();
                let row_colors = list_colors;
                div()
                    .id(SharedString::from(format!("rebase-step-{index}")))
                    .px_2()
                    .py_1()
                    .flex()
                    .flex_col()
                    .bg(row_colors.panel_background)
                    .border_b_1()
                    .border_color(row_colors.border)
                    .child(format!("{verb}  {arguments}"))
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(file_action_button(
                                "Change action",
                                &row_colors,
                                cx,
                                move |app, cx| {
                                    if let Some(current) = app.rebase_plan.get_mut(index) {
                                        current.action = current.action.next();
                                    }
                                    cx.notify();
                                },
                            ))
                            .child(file_action_button("Up", &row_colors, cx, move |app, cx| {
                                if can_up {
                                    app.rebase_plan.swap(index, index - 1);
                                }
                                cx.notify();
                            }))
                            .child(file_action_button(
                                "Down",
                                &row_colors,
                                cx,
                                move |app, cx| {
                                    if can_down {
                                        app.rebase_plan.swap(index, index + 1);
                                    }
                                    cx.notify();
                                },
                            ))
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(row_colors.text_secondary)
                                    .child(format!("next: {next_verb}")),
                            ),
                    )
            }))
    }
}
