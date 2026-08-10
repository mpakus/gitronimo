//! History view: virtualized commit rows, graph canvas, search, inspector.

use gpui::{ClickEvent, PathBuilder, canvas, div, list, point, prelude::*, px};
use ui_kit::ThemeColors;

use git_domain::{HistoryReference, WorktreeRepository};

use crate::app_state::GitronimoApp;
use crate::views::components::{file_action_button, state_panel};

impl GitronimoApp {
    pub(crate) fn history_row_count(&self) -> usize {
        let search = self.history_search.to_lowercase();
        self.history
            .iter()
            .filter(|commit| {
                search.is_empty()
                    || commit.oid.contains(&search)
                    || String::from_utf8_lossy(&commit.subject)
                        .to_lowercase()
                        .contains(&search)
                    || String::from_utf8_lossy(&commit.author.name)
                        .to_lowercase()
                        .contains(&search)
            })
            .count()
    }

    #[allow(clippy::too_many_lines)]
    pub(crate) fn history_view(
        &self,
        repository: &WorktreeRepository,
        colors: &ThemeColors,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let search = self.history_search.to_lowercase();
        let rows = self
            .history
            .iter()
            .enumerate()
            .filter(|(_, commit)| {
                search.is_empty()
                    || commit.oid.contains(&search)
                    || String::from_utf8_lossy(&commit.subject)
                        .to_lowercase()
                        .contains(&search)
                    || String::from_utf8_lossy(&commit.author.name)
                        .to_lowercase()
                        .contains(&search)
            })
            .map(|(history_index, commit)| {
                let graph_row = self.history_rows.get(history_index);
                let lane = graph_row.map_or(0, |row| row.lane);
                let parent_lanes = graph_row.map_or_else(Vec::new, |row| row.parent_lanes.clone());
                let decorations = self
                    .history_decorations
                    .iter()
                    .filter(|decoration| decoration.target == commit.oid)
                    .map(|decoration| String::from_utf8_lossy(&decoration.name).to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                (
                    history_index,
                    lane,
                    parent_lanes,
                    format!(
                        "{} ● {} · {} — {} {}",
                        "│ ".repeat(lane),
                        String::from_utf8_lossy(&commit.author.name),
                        commit.author.timestamp,
                        String::from_utf8_lossy(&commit.subject),
                        decorations
                    ),
                )
            })
            .collect::<Vec<_>>();
        let selected = self.selected_history;
        let list_colors = *colors;
        let list_repository = repository.clone();
        let commit_list = list(
            self.history_list_state.clone(),
            cx.processor(move |_app, visible_index: usize, _, cx| {
                let (history_index, lane, parent_lanes, label) = rows[visible_index].clone();
                let repository = list_repository.clone();
                div()
                    .id(visible_index)
                    .h(px(28.0))
                    .px_2()
                    .flex()
                    .items_center()
                    .bg(if selected == Some(history_index) {
                        list_colors.selection
                    } else {
                        list_colors.panel_background
                    })
                    .border_b_1()
                    .border_color(list_colors.border)
                    .child(
                        canvas(
                            move |bounds, _, _| {
                                let lane_offset = u8::try_from(lane.min(100)).unwrap_or(100);
                                let x = bounds.origin.x + px(10.0 + f32::from(lane_offset) * 8.0);
                                let mut path = PathBuilder::stroke(px(2.0));
                                path.move_to(point(x, bounds.origin.y));
                                let center_y = bounds.origin.y + bounds.size.height / 2.0;
                                path.line_to(point(x, center_y));
                                for parent_lane in &parent_lanes {
                                    let parent_offset =
                                        u8::try_from((*parent_lane).min(100)).unwrap_or(100);
                                    let parent_x =
                                        bounds.origin.x + px(10.0 + f32::from(parent_offset) * 8.0);
                                    path.move_to(point(x, center_y));
                                    path.line_to(point(
                                        parent_x,
                                        bounds.origin.y + bounds.size.height,
                                    ));
                                }
                                if parent_lanes.is_empty() {
                                    path.line_to(point(x, bounds.origin.y + bounds.size.height));
                                }
                                path.build().ok()
                            },
                            move |_, path, window, _| {
                                if let Some(path) = path {
                                    window.paint_path(
                                        path,
                                        list_colors.graph_lanes
                                            [lane % list_colors.graph_lanes.len()],
                                    );
                                }
                            },
                        )
                        .w(px(28.0))
                        .h_full(),
                    )
                    .cursor_pointer()
                    .on_click(cx.listener(move |app, event: &ClickEvent, _, cx| {
                        app.select_history_commit(history_index, repository.clone(), cx);
                        if event.click_count() >= 2 {
                            app.show_commit_detail(&repository, history_index, cx);
                        }
                    }))
                    .child(label)
                    .into_any_element()
            }),
        )
        .h_full();
        let detail = self
            .selected_history
            .and_then(|index| self.history.get(index))
            .map_or_else(
                || {
                    state_panel(
                        "No commit selected",
                        "Choose a commit to inspect its details.",
                        colors.text_muted,
                        colors,
                    )
                    .into_any_element()
                },
                |commit| {
                    div()
                        .p_3()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(div().text_sm().text_color(colors.text_muted).child(format!(
                            "Author: {}",
                            String::from_utf8_lossy(&commit.author.name)
                        )))
                        .child(
                            div()
                                .text_sm()
                                .text_color(colors.text_muted)
                                .child(format!("Date: {}", commit.author.timestamp)),
                        )
                        .child(
                            div()
                                .text_sm()
                                .child(String::from_utf8_lossy(&commit.subject).to_string()),
                        )
                        .child(
                            div()
                                .text_sm()
                                .text_color(colors.text_secondary)
                                .child(String::from_utf8_lossy(&commit.body).to_string()),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(colors.text_muted)
                                .child(format!("SHA: {}", commit.oid)),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(colors.text_muted)
                                .child(format!("Parents: {}", commit.parents.join(" "))),
                        )
                        .child(div().text_xs().text_color(colors.text_muted).child(format!(
                            "Changed: {}",
                            self.history_paths
                                .iter()
                                .map(|path| String::from_utf8_lossy(&path.0))
                                .collect::<Vec<_>>()
                                .join(", ")
                        )))
                        .children(self.history_diff.as_ref().map(|diff| {
                            div().text_xs().text_color(colors.text_muted).child(format!(
                                "Selected diff: {} file(s){}",
                                diff.diff.files.len(),
                                if diff.truncated { " (truncated)" } else { "" }
                            ))
                        }))
                        .into_any_element()
                },
            );
        let load_more = self.history_next.as_ref().map(|before| {
            let repository = repository.clone();
            let before = before.clone();
            file_action_button("Load more history", colors, cx, move |app, cx| {
                app.load_history(repository.clone(), Some(before.clone()), cx);
            })
        });
        let current_repository = repository.clone();
        let all_repository = repository.clone();
        div()
            .flex()
            .flex_col()
            .child(
                div()
                    .px_3()
                    .py_2()
                    .flex()
                    .items_center()
                    .gap_2()
                    .border_b_1()
                    .border_color(colors.border)
                    .child(file_action_button(
                        "Current branch",
                        colors,
                        cx,
                        move |app, cx| {
                            app.change_history_reference(
                                HistoryReference::Current,
                                current_repository.clone(),
                                cx,
                            );
                        },
                    ))
                    .child(file_action_button(
                        "All refs",
                        colors,
                        cx,
                        move |app, cx| {
                            app.change_history_reference(
                                HistoryReference::All,
                                all_repository.clone(),
                                cx,
                            );
                        },
                    ))
                    .child(file_action_button(
                        "Branch or tag…",
                        colors,
                        cx,
                        |_, cx| GitronimoApp::prompt_history_reference(cx),
                    ))
                    .child(file_action_button("Search history", colors, cx, |_, cx| {
                        GitronimoApp::prompt_history_search(cx);
                    }))
                    .child(file_action_button("Reveal HEAD", colors, cx, |app, cx| {
                        app.reveal_history_head(cx);
                    }))
                    .child(file_action_button(
                        "Copy selected OID",
                        colors,
                        cx,
                        GitronimoApp::copy_selected_history_oid,
                    ))
                    .child(file_action_button(
                        "New branch from commit…",
                        colors,
                        cx,
                        |_, cx| GitronimoApp::prompt_branch_from_selected(cx),
                    )),
            )
            .child(
                div()
                    .flex_1()
                    .flex()
                    .items_start()
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(300.0))
                            .flex()
                            .flex_col()
                            .child(commit_list)
                            .children(load_more),
                    )
                    .child(div().w(px(1.0)).h_full().bg(colors.border))
                    .child(div().flex_1().min_w(px(360.0)).child(detail)),
            )
    }
}
