//! Conflicts view: resolve conflicted files during a merge or rebase by
//! viewing the marker file and taking either side.

use gpui::{SharedString, div, prelude::*};
use ui_kit::ThemeColors;

use git_domain::{ConflictSide, WorktreeRepository};

use crate::app_state::{GitronimoApp, RepositoryView, ShellState};
use crate::views::components::{file_action_button, status_path};

impl GitronimoApp {
    #[allow(clippy::too_many_lines)]
    pub(crate) fn conflicts_view(
        &self,
        _repository: &WorktreeRepository,
        colors: &ThemeColors,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let conflicts = self.status_groups().conflicts;
        let list_colors = *colors;
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(div().text_xl().child("Conflicts"))
            .child(file_action_button("Working Copy", colors, cx, |app, cx| {
                app.navigate_to(RepositoryView::WorkingCopy, cx);
            }))
            .child(file_action_button("Refresh", colors, cx, |app, cx| {
                app.conflict_path = None;
                app.conflict_content = None;
                if let ShellState::Repository(repository) = &app.state {
                    app.load_working_copy(repository.clone(), cx);
                }
            }))
            .children(conflicts.iter().map(|entry| {
                let path = status_path(entry).clone();
                let path_label = String::from_utf8_lossy(&path.0).to_string();
                let ours_path = path.clone();
                let theirs_path = path.clone();
                let view_path = path.clone();
                let colors = list_colors;
                div()
                    .id(SharedString::from(format!("conflict-{path_label}")))
                    .px_2()
                    .py_1()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .bg(colors.panel_background)
                    .border_b_1()
                    .border_color(colors.border)
                    .child(format!("UU  {path_label}"))
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(file_action_button(
                                "Take ours",
                                &colors,
                                cx,
                                move |_, cx| {
                                    GitronimoApp::resolve_conflict(
                                        ours_path.clone(),
                                        ConflictSide::Ours,
                                        cx,
                                    );
                                },
                            ))
                            .child(file_action_button(
                                "Take theirs",
                                &colors,
                                cx,
                                move |_, cx| {
                                    GitronimoApp::resolve_conflict(
                                        theirs_path.clone(),
                                        ConflictSide::Theirs,
                                        cx,
                                    );
                                },
                            ))
                            .child(file_action_button("View", &colors, cx, move |app, cx| {
                                let ShellState::Repository(repository) = &app.state else {
                                    return;
                                };
                                let repository = repository.clone();
                                GitronimoApp::view_conflict(repository, view_path.clone(), cx);
                            })),
                    )
            }))
            .child(if let Some(content) = &self.conflict_content {
                let path_label = self
                    .conflict_path
                    .as_ref()
                    .map(|path| String::from_utf8_lossy(&path.0).to_string())
                    .unwrap_or_default();
                let text = String::from_utf8_lossy(content).into_owned();
                div()
                    .id("conflict-content")
                    .px_2()
                    .py_1()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .bg(list_colors.panel_background)
                    .border_1()
                    .border_color(list_colors.border)
                    .child(
                        div()
                            .text_sm()
                            .child(format!("{path_label} (working copy)")),
                    )
                    .child(
                        div()
                            .font_family("Monaco")
                            .whitespace_nowrap()
                            .text_sm()
                            .child(text),
                    )
            } else {
                div()
                    .id("conflict-empty")
                    .child("Select a conflicted file to view its marker content.")
            })
    }
}
