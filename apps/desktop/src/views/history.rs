//! History view: virtualized commit rows, graph canvas, search, inspector.

use gpui::{
    Bounds, ClickEvent, PathBuilder, Pixels, Rgba, SharedString, canvas, div, list, point,
    prelude::*, px,
};
use ui_kit::ThemeColors;

use git_domain::{DiffLineKind, HistoryReference, WorktreeRepository};

use crate::app_state::{ChoicePromptKind, GitronimoApp, HistoryDetailMode};
use crate::views::components::{
    centered_empty_state, file_action_button, month_group_label, segmented_detail_toggle,
    short_calendar_date,
};

#[derive(Clone)]
enum HistoryListItem {
    MonthHeader(String),
    Commit {
        history_index: usize,
        lane: usize,
        parent_lanes: Vec<usize>,
        active_lanes: Vec<usize>,
        author: String,
        initials: String,
        when: String,
        short_oid: String,
        subject: String,
        refs: Vec<RefPill>,
    },
}

#[derive(Clone)]
struct RefPill {
    label: String,
    is_head: bool,
    is_remote: bool,
}

impl GitronimoApp {
    pub(crate) fn history_row_count(&self) -> usize {
        self.history_list_items().len()
    }

    fn history_scope_title(&self) -> String {
        match &self.history_reference {
            HistoryReference::Current => "Current Branch".into(),
            HistoryReference::All => "All Branches, Remotes, Tags".into(),
            HistoryReference::Named(name) => name.clone(),
        }
    }

    fn decorations_for_oid(&self, oid: &str) -> Vec<RefPill> {
        let mut pills = Vec::new();
        for decoration in &self.history_decorations {
            if decoration.target != oid {
                continue;
            }
            let label = String::from_utf8_lossy(&decoration.name).into_owned();
            let is_head = label == "HEAD";
            let is_remote = label.contains('/');
            pills.push(RefPill {
                label,
                is_head,
                is_remote,
            });
        }
        pills.sort_by(|left, right| {
            right
                .is_head
                .cmp(&left.is_head)
                .then_with(|| left.is_remote.cmp(&right.is_remote))
                .then_with(|| left.label.cmp(&right.label))
        });
        pills
    }

    fn history_list_items(&self) -> Vec<HistoryListItem> {
        let search = self.history_search.to_lowercase();
        let mut items = Vec::new();
        let mut last_month: Option<String> = None;
        for (history_index, commit) in self.history.iter().enumerate() {
            let author = String::from_utf8_lossy(&commit.author.name).into_owned();
            let subject = String::from_utf8_lossy(&commit.subject).into_owned();
            let matches = search.is_empty()
                || commit.oid.to_lowercase().contains(&search)
                || subject.to_lowercase().contains(&search)
                || author.to_lowercase().contains(&search);
            if !matches {
                continue;
            }
            let month = month_group_label(commit.author.timestamp);
            if last_month.as_ref() != Some(&month) {
                items.push(HistoryListItem::MonthHeader(month.clone()));
                last_month = Some(month);
            }
            let graph_row = self.history_rows.get(history_index);
            let lane = graph_row.map_or(0, |row| row.lane);
            let parent_lanes = graph_row.map_or_else(Vec::new, |row| row.parent_lanes.clone());
            let active_lanes = graph_row.map_or_else(|| vec![0], |row| row.active_lanes.clone());
            let initials = author
                .chars()
                .next()
                .map_or_else(|| "?".into(), |ch| ch.to_uppercase().to_string());
            items.push(HistoryListItem::Commit {
                history_index,
                lane,
                parent_lanes,
                active_lanes,
                author,
                initials,
                when: short_calendar_date(commit.author.timestamp),
                short_oid: commit.oid.chars().take(8).collect(),
                subject,
                refs: self.decorations_for_oid(&commit.oid),
            });
        }
        items
    }

    #[allow(clippy::too_many_lines)]
    pub(crate) fn history_view(
        &self,
        repository: &WorktreeRepository,
        colors: &ThemeColors,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let items = self.history_list_items();
        let selected = self.selected_history;
        let list_colors = *colors;
        let list_repository = repository.clone();
        let commit_list = list(
            self.history_list_state.clone(),
            cx.processor(move |_app, visible_index: usize, _, cx| {
                match items[visible_index].clone() {
                    HistoryListItem::MonthHeader(label) => div()
                        .id(("history-month", visible_index))
                        .w_full()
                        .h(px(26.0))
                        .px_3()
                        .flex()
                        .items_center()
                        .border_b_1()
                        .border_color(list_colors.separator)
                        .child(
                            div()
                                .text_xs()
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .text_color(list_colors.text_muted)
                                .child(label),
                        )
                        .into_any_element(),
                    HistoryListItem::Commit {
                        history_index,
                        lane,
                        parent_lanes,
                        active_lanes,
                        author,
                        initials,
                        when,
                        short_oid,
                        subject,
                        refs,
                    } => {
                        let repository = list_repository.clone();
                        let selected_row = selected == Some(history_index);
                        let primary = if selected_row {
                            list_colors.panel_background
                        } else {
                            list_colors.text_primary
                        };
                        let secondary = if selected_row {
                            list_colors.panel_background
                        } else {
                            list_colors.text_secondary
                        };
                        let muted = if selected_row {
                            list_colors.panel_background
                        } else {
                            list_colors.text_muted
                        };
                        let lane_span = active_lanes
                            .iter()
                            .copied()
                            .chain(parent_lanes.iter().copied())
                            .chain(std::iter::once(lane))
                            .max()
                            .unwrap_or(0)
                            .saturating_add(1);
                        let graph_width = (16.0
                            + f32::from(u16::try_from(lane_span.min(12)).unwrap_or(12)) * 12.0)
                            .clamp(40.0, 132.0);
                        let mut row = div()
                            .id(("history-commit", visible_index))
                            .w_full()
                            .h(px(56.0))
                            .px_3()
                            .pt_2()
                            .pb_1p5()
                            .flex()
                            .items_start()
                            .gap_2()
                            .border_b_1()
                            .border_color(if selected_row {
                                list_colors.accent
                            } else {
                                list_colors.separator
                            });
                        if selected_row {
                            row = row.bg(list_colors.accent);
                        }
                        row.child(history_graph_canvas(
                            lane,
                            parent_lanes,
                            active_lanes,
                            graph_width,
                            selected_row,
                            &list_colors,
                        ))
                        .child(
                            div()
                                .w(px(22.0))
                                .h(px(22.0))
                                .rounded_full()
                                .flex_shrink_0()
                                .flex()
                                .items_center()
                                .justify_center()
                                .bg(if selected_row {
                                    list_colors.panel_background
                                } else {
                                    list_colors.raised_background
                                })
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(gpui::FontWeight::SEMIBOLD)
                                        .text_color(if selected_row {
                                            list_colors.accent
                                        } else {
                                            list_colors.text_primary
                                        })
                                        .child(initials),
                                ),
                        )
                        .child(
                            div()
                                .flex_1()
                                .w_full()
                                .min_w(px(0.0))
                                .overflow_hidden()
                                .flex()
                                .flex_col()
                                .gap_0p5()
                                .child(
                                    div()
                                        .w_full()
                                        .flex()
                                        .items_center()
                                        .justify_between()
                                        .gap_2()
                                        .child(
                                            div()
                                                .flex_1()
                                                .min_w(px(0.0))
                                                .overflow_hidden()
                                                .whitespace_nowrap()
                                                .text_ellipsis()
                                                .text_xs()
                                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                                .text_color(primary)
                                                .child(author),
                                        )
                                        .child(
                                            div()
                                                .flex_shrink_0()
                                                .text_xs()
                                                .text_color(muted)
                                                .child(when),
                                        ),
                                )
                                .child(
                                    div()
                                        .w_full()
                                        .flex()
                                        .items_center()
                                        .gap_1p5()
                                        .overflow_hidden()
                                        .child(
                                            div()
                                                .flex_shrink_0()
                                                .font_family("Menlo")
                                                .text_xs()
                                                .text_color(muted)
                                                .child(short_oid),
                                        )
                                        .child(
                                            div()
                                                .flex_1()
                                                .min_w(px(0.0))
                                                .overflow_hidden()
                                                .whitespace_nowrap()
                                                .text_ellipsis()
                                                .text_xs()
                                                .text_color(secondary)
                                                .child(subject),
                                        )
                                        .children(refs.iter().map(|pill| {
                                            ref_label(pill, selected_row, &list_colors)
                                        })),
                                ),
                        )
                        .cursor_pointer()
                        .on_click(cx.listener(move |app, event: &ClickEvent, _, cx| {
                            app.select_history_commit(history_index, repository.clone(), cx);
                            if event.click_count() >= 2 {
                                app.show_commit_detail(&repository, history_index, cx);
                            }
                        }))
                        .into_any_element()
                    }
                }
            }),
        )
        .w_full()
        .h_full();

        let detail = self.history_detail_pane(repository, colors, cx);
        let load_more = self.history_next.as_ref().map(|before| {
            let repository = repository.clone();
            let before = before.clone();
            file_action_button("Load more history", colors, cx, move |app, cx| {
                app.load_history(repository.clone(), Some(before.clone()), cx);
            })
        });

        div()
            .flex()
            .flex_col()
            .flex_1()
            .h_full()
            .overflow_hidden()
            .child(self.history_scope_header(colors, cx))
            .child(
                div()
                    .flex_1()
                    .h_full()
                    .overflow_hidden()
                    .flex()
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(320.0))
                            .h_full()
                            .overflow_hidden()
                            .flex()
                            .flex_col()
                            .child(div().flex_1().overflow_hidden().child(commit_list))
                            .children(load_more),
                    )
                    .child(div().w(px(1.0)).h_full().bg(colors.border))
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(360.0))
                            .h_full()
                            .overflow_hidden()
                            .child(detail),
                    ),
            )
    }

    fn history_scope_header(
        &self,
        colors: &ThemeColors,
        cx: &mut gpui::Context<Self>,
    ) -> gpui::AnyElement {
        let title = self.history_scope_title();
        div()
            .h(px(36.0))
            .px_3()
            .flex()
            .items_center()
            .justify_between()
            .border_b_1()
            .border_color(colors.border)
            .child(
                div()
                    .text_sm()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(colors.text_primary)
                    .child(title),
            )
            .child(
                div()
                    .id("history-filter")
                    .px_2()
                    .py_1()
                    .rounded(px(4.0))
                    .text_xs()
                    .text_color(colors.text_secondary)
                    .cursor_pointer()
                    .hover(|style| style.bg(colors.selection))
                    .on_click(cx.listener(|app, _, _, cx| {
                        app.begin_choice_prompt(ChoicePromptKind::HistoryFilter, cx);
                    }))
                    .child("Filter"),
            )
            .into_any_element()
    }

    #[allow(clippy::too_many_lines)]
    fn history_detail_pane(
        &self,
        repository: &WorktreeRepository,
        colors: &ThemeColors,
        cx: &mut gpui::Context<Self>,
    ) -> gpui::AnyElement {
        let Some(commit) = self
            .selected_history
            .and_then(|index| self.history.get(index))
        else {
            return centered_empty_state(
                "No commit selected",
                "Choose a commit to inspect its details.",
                colors,
            );
        };
        let repository_for_toggle = repository.clone();
        let changeset_repo = repository.clone();
        let tree_repo = repository.clone();
        let author = String::from_utf8_lossy(&commit.author.name).into_owned();
        let author_email = String::from_utf8_lossy(&commit.author.email).into_owned();
        let committer = String::from_utf8_lossy(&commit.committer.name).into_owned();
        let committer_email = String::from_utf8_lossy(&commit.committer.email).into_owned();
        let initials = author
            .chars()
            .next()
            .map_or_else(|| "?".into(), |ch| ch.to_uppercase().to_string());
        let subject = String::from_utf8_lossy(&commit.subject).into_owned();
        let body = String::from_utf8_lossy(&commit.body).into_owned();
        let short_oid = commit.oid.chars().take(8).collect::<String>();
        let parent = commit
            .parents
            .first()
            .map_or_else(|| "—".into(), |oid| oid.chars().take(8).collect::<String>());
        let refs = self.decorations_for_oid(&commit.oid);
        let (file_count, additions, deletions) = self.history_change_summary();

        div()
            .flex()
            .flex_col()
            .h_full()
            .overflow_hidden()
            .child(
                div()
                    .px_3()
                    .py_2()
                    .flex()
                    .items_center()
                    .justify_between()
                    .border_b_1()
                    .border_color(colors.border)
                    .child(
                        div()
                            .text_xs()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(colors.text_primary)
                            .child(short_oid),
                    )
                    .child(segmented_detail_toggle(
                        "Changeset",
                        "Tree",
                        self.history_detail_mode == HistoryDetailMode::Changeset,
                        colors,
                        cx,
                        move |app, cx| {
                            app.toggle_history_detail_mode(
                                HistoryDetailMode::Changeset,
                                changeset_repo.clone(),
                                cx,
                            );
                        },
                        move |app, cx| {
                            app.toggle_history_detail_mode(
                                HistoryDetailMode::Tree,
                                tree_repo.clone(),
                                cx,
                            );
                        },
                    )),
            )
            .child(
                div()
                    .px_3()
                    .py_2()
                    .flex()
                    .gap_3()
                    .border_b_1()
                    .border_color(colors.separator)
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(meta_row(
                                "Author",
                                format!("{author} <{author_email}>"),
                                colors,
                            ))
                            .child(meta_row(
                                "Author Date",
                                short_calendar_date(commit.author.timestamp),
                                colors,
                            ))
                            .child(meta_row(
                                "Committer",
                                format!("{committer} <{committer_email}>"),
                                colors,
                            ))
                            .child(meta_row(
                                "Committer Date",
                                short_calendar_date(commit.committer.timestamp),
                                colors,
                            ))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .gap_2()
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(colors.text_muted)
                                            .child("Refs"),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .flex_wrap()
                                            .gap_1()
                                            .justify_end()
                                            .children(refs.into_iter().map(|pill| {
                                                ref_label(&pill, false, colors)
                                            })),
                                    ),
                            )
                            .child(meta_row("Commit Hash", commit.oid.clone(), colors))
                            .child(meta_row("Parent Hash", parent, colors)),
                    )
                    .child(
                        div()
                            .w(px(56.0))
                            .h(px(56.0))
                            .rounded_full()
                            .flex()
                            .items_center()
                            .justify_center()
                            .bg(colors.raised_background)
                            .child(
                                div()
                                    .text_lg()
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .text_color(colors.text_primary)
                                    .child(initials),
                            ),
                    ),
            )
            .child(
                div()
                    .px_3()
                    .py_2()
                    .border_b_1()
                    .border_color(colors.separator)
                    .child(
                        div()
                            .text_sm()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(colors.text_primary)
                            .child(subject),
                    )
                    .when(!body.trim().is_empty(), |this| {
                        this.child(
                            div()
                                .mt_1()
                                .text_xs()
                                .text_color(colors.text_secondary)
                                .child(body),
                        )
                    }),
            )
            .when(
                self.history_detail_mode == HistoryDetailMode::Changeset,
                |this| {
                    this.child(
                        div()
                            .px_3()
                            .py_1p5()
                            .flex()
                            .items_center()
                            .justify_between()
                            .border_b_1()
                            .border_color(colors.separator)
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(colors.text_muted)
                                    .child(format!(
                                        "Showing {file_count} changed file{} with {additions} additions and {deletions} deletions",
                                        if file_count == 1 { "" } else { "s" }
                                    )),
                            ),
                    )
                    .child(
                        div()
                            .flex_1()
                            .overflow_hidden()
                            .child(self.history_changeset_files(colors)),
                    )
                },
            )
            .when(
                self.history_detail_mode == HistoryDetailMode::Tree,
                |this| {
                    this.child(
                        div().flex_1().overflow_hidden().child(self.tree_panel(
                            &repository_for_toggle,
                            colors,
                            cx,
                        )),
                    )
                },
            )
            .into_any_element()
    }

    fn history_change_summary(&self) -> (usize, usize, usize) {
        let Some(loaded) = &self.history_diff else {
            return (self.history_paths.len(), 0, 0);
        };
        let mut additions = 0usize;
        let mut deletions = 0usize;
        for file in &loaded.diff.files {
            for hunk in &file.hunks {
                for line in &hunk.lines {
                    match line.kind {
                        DiffLineKind::Addition => additions += 1,
                        DiffLineKind::Removal => deletions += 1,
                        DiffLineKind::Context => {}
                    }
                }
            }
        }
        (loaded.diff.files.len(), additions, deletions)
    }

    fn history_changeset_files(&self, colors: &ThemeColors) -> gpui::AnyElement {
        let Some(loaded) = &self.history_diff else {
            if self.history_paths.is_empty() {
                return div()
                    .p_3()
                    .text_xs()
                    .text_color(colors.text_muted)
                    .child("Loading changes…")
                    .into_any_element();
            }
            return div()
                .flex()
                .flex_col()
                .children(self.history_paths.iter().enumerate().map(|(index, path)| {
                    let path = String::from_utf8_lossy(&path.0).into_owned();
                    changeset_file_row(index, "changed", path, colors)
                }))
                .into_any_element();
        };
        if loaded.diff.files.is_empty() {
            return div()
                .p_3()
                .text_xs()
                .text_color(colors.text_muted)
                .child("No file changes in this commit.")
                .into_any_element();
        }
        div()
            .flex()
            .flex_col()
            .overflow_hidden()
            .children(loaded.diff.files.iter().enumerate().map(|(index, file)| {
                let path = file
                    .new_path
                    .as_ref()
                    .or(file.old_path.as_ref())
                    .map(|path| String::from_utf8_lossy(&path.0).into_owned())
                    .unwrap_or_default();
                let status = match (&file.old_path, &file.new_path) {
                    (None, Some(_)) => "added",
                    (Some(_), None) => "deleted",
                    _ => "modified",
                };
                changeset_file_row(index, status, path, colors)
            }))
            .into_any_element()
    }
}

fn history_graph_canvas(
    lane: usize,
    parent_lanes: Vec<usize>,
    active_lanes: Vec<usize>,
    graph_width: f32,
    selected_row: bool,
    colors: &ThemeColors,
) -> gpui::AnyElement {
    let palette = colors.graph_lanes;
    let row_bg = if selected_row {
        colors.accent
    } else {
        colors.panel_background
    };
    canvas(
        move |bounds, _, _| {
            build_history_graph_layers(bounds, lane, &parent_lanes, &active_lanes, &palette, row_bg)
        },
        move |_, layers, window, _| {
            for (path, color) in layers {
                window.paint_path(path, color);
            }
        },
    )
    .w(px(graph_width))
    .h_full()
    .into_any_element()
}

fn lane_x(bounds: Bounds<Pixels>, lane: usize) -> Pixels {
    let offset = u8::try_from(lane.min(24)).unwrap_or(24);
    bounds.origin.x + px(10.0 + f32::from(offset) * 12.0)
}

fn build_history_graph_layers(
    bounds: Bounds<Pixels>,
    lane: usize,
    parent_lanes: &[usize],
    active_lanes: &[usize],
    palette: &[Rgba; 6],
    row_bg: Rgba,
) -> Vec<(gpui::Path<Pixels>, Rgba)> {
    let mut layers = Vec::new();
    let top = bounds.origin.y;
    let bottom = bounds.origin.y + bounds.size.height;
    let mid = top + bounds.size.height / 2.0;
    let node_x = lane_x(bounds, lane);

    for &active in active_lanes {
        let color = palette[active % palette.len()];
        let x = lane_x(bounds, active);
        let mut path = PathBuilder::stroke(px(2.0));
        path.move_to(point(x, top));
        if active == lane {
            path.line_to(point(x, mid));
        } else {
            path.line_to(point(x, bottom));
        }
        if let Ok(built) = path.build() {
            layers.push((built, color));
        }
    }

    for &parent in parent_lanes {
        let color = palette[parent % palette.len()];
        let parent_x = lane_x(bounds, parent);
        let mut path = PathBuilder::stroke(px(2.0));
        path.move_to(point(node_x, mid));
        path.line_to(point(parent_x, bottom));
        if let Ok(built) = path.build() {
            layers.push((built, color));
        }
    }

    if parent_lanes.is_empty() {
        let color = palette[lane % palette.len()];
        let mut path = PathBuilder::stroke(px(2.0));
        path.move_to(point(node_x, mid));
        path.line_to(point(node_x, bottom));
        if let Ok(built) = path.build() {
            layers.push((built, color));
        }
    }

    // Node ring + fill (approach adapted from rgitui MIT graph paint).
    if let Some(ring) = filled_circle(node_x, mid, 5.5) {
        layers.push((ring, row_bg));
    }
    if let Some(dot) = filled_circle(node_x, mid, 3.5) {
        layers.push((dot, palette[lane % palette.len()]));
    }
    layers
}

fn filled_circle(center_x: Pixels, center_y: Pixels, radius: f32) -> Option<gpui::Path<Pixels>> {
    let steps = 24_u8;
    let mut path = PathBuilder::fill();
    for step in 0..steps {
        let angle = f32::from(step) * std::f32::consts::TAU / f32::from(steps);
        let x = center_x + px(radius * angle.cos());
        let y = center_y + px(radius * angle.sin());
        if step == 0 {
            path.move_to(point(x, y));
        } else {
            path.line_to(point(x, y));
        }
    }
    path.close();
    path.build().ok()
}

fn meta_row(label: &str, value: String, colors: &ThemeColors) -> gpui::AnyElement {
    div()
        .flex()
        .items_start()
        .justify_between()
        .gap_3()
        .child(
            div()
                .text_xs()
                .text_color(colors.text_muted)
                .child(label.to_owned()),
        )
        .child(
            div()
                .flex_1()
                .text_right()
                .text_xs()
                .text_color(colors.text_primary)
                .child(value),
        )
        .into_any_element()
}

fn ref_label(pill: &RefPill, selected_row: bool, colors: &ThemeColors) -> gpui::AnyElement {
    let label = truncate_ref_label(&pill.label, 32);
    let color = if selected_row {
        colors.panel_background
    } else if pill.is_head {
        colors.accent
    } else if pill.is_remote {
        colors.text_muted
    } else {
        colors.accent
    };
    div()
        .flex_shrink_0()
        .max_w(px(180.0))
        .overflow_hidden()
        .whitespace_nowrap()
        .text_ellipsis()
        .text_xs()
        .font_weight(if pill.is_head || !pill.is_remote {
            gpui::FontWeight::MEDIUM
        } else {
            gpui::FontWeight::NORMAL
        })
        .text_color(color)
        .child(label)
        .into_any_element()
}

fn truncate_ref_label(label: &str, max_chars: usize) -> String {
    let count = label.chars().count();
    if count <= max_chars {
        return label.to_owned();
    }
    let keep = max_chars.saturating_sub(1);
    let mut truncated: String = label.chars().take(keep).collect();
    truncated.push('…');
    truncated
}

fn changeset_file_row(
    index: usize,
    status: &'static str,
    path: String,
    colors: &ThemeColors,
) -> gpui::AnyElement {
    let status_color = match status {
        "added" => colors.success,
        "deleted" => colors.danger,
        _ => colors.accent,
    };
    div()
        .id(SharedString::from(format!("history-file-{index}")))
        .h(px(26.0))
        .px_3()
        .flex()
        .items_center()
        .gap_2()
        .border_b_1()
        .border_color(colors.separator)
        .child(
            div()
                .w(px(56.0))
                .text_xs()
                .font_weight(gpui::FontWeight::MEDIUM)
                .text_color(status_color)
                .child(status),
        )
        .child(
            div()
                .flex_1()
                .overflow_hidden()
                .whitespace_nowrap()
                .text_ellipsis()
                .text_xs()
                .text_color(colors.text_secondary)
                .child(path),
        )
        .into_any_element()
}
