//! Diff viewer: per-line rendered diff rows with line selection for partial
//! staging or discarding, plus hunk-level stage/unstage actions.

use gpui::{AnyElement, SharedString, div, prelude::*, px};
use ui_kit::ThemeColors;

use git_domain::{DiffLine, DiffLineKind};

use crate::app_state::GitronimoApp;
use crate::views::components::file_action_button;

impl GitronimoApp {
    pub(crate) fn diff_view(
        &self,
        colors: &ThemeColors,
        cx: &mut gpui::Context<Self>,
    ) -> Option<AnyElement> {
        self.loaded_diff.as_ref().map(|loaded| {
            let staged_diff = matches!(&self.selected_diff, Some((_, true)));
            let is_binary = loaded.diff.files.iter().any(|file| file.binary);
            let can_mutate_hunks = !self.mutation_in_flight
                && !loaded.truncated
                && !is_binary
                && self.selected_diff.is_some();
            let can_select_lines = can_mutate_hunks && !staged_diff;
            let selection_count = self.selected_diff_lines.len();
            let (hunk_count, total_additions, total_deletions, code_rows) =
                self.diff_code_rows(can_select_lines, colors, cx);
            let file_name = loaded
                .diff
                .files
                .first()
                .and_then(|f| f.new_path.as_ref())
                .map_or_else(|| "No file selected".into(), |p| String::from_utf8_lossy(&p.0).into_owned());
            let chunk_info = if hunk_count > 0 {
                format!(
                    "{hunk_count} chunk{}, {total_additions} insertion{}, {total_deletions} deletion{}",
                    if hunk_count == 1 { "" } else { "s" },
                    if total_additions == 1 { "" } else { "s" },
                    if total_deletions == 1 { "" } else { "s" },
                )
            } else {
                String::new()
            };
            div()
                .flex()
                .flex_col()
                .gap_0()
                .bg(colors.panel_background)
                .child(Self::diff_header(
                    file_name,
                    staged_diff,
                    chunk_info,
                    colors,
                ))
                .children(Self::selection_controls(
                    can_select_lines,
                    selection_count,
                    colors,
                    cx,
                ))
                .child(if is_binary {
                    div()
                        .p_3()
                        .child("Binary file changed")
                        .into_any_element()
                } else {
                    div().p_2().children(code_rows).into_any_element()
                })
                .children(loaded.truncated.then(|| {
                    div()
                        .p_2()
                        .child("Diff truncated.")
                        .child(file_action_button(
                            "Load full diff",
                            colors,
                            cx,
                            |app, cx| {
                                app.load_full_diff(cx);
                            },
                        ))
                }))
                .into_any_element()
        })
    }

    fn diff_header(
        file_name: String,
        staged_diff: bool,
        chunk_info: String,
        colors: &ThemeColors,
    ) -> AnyElement {
        div()
            .flex()
            .flex_col()
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
                            .child(file_name),
                    )
                    .child(Self::staged_unstaged_tabs(staged_diff, colors)),
            )
            .when(!chunk_info.is_empty(), |this| {
                this.child(
                    div()
                        .px_3()
                        .py_1()
                        .text_xs()
                        .text_color(colors.text_muted)
                        .child(chunk_info),
                )
            })
            .into_any_element()
    }

    fn selection_controls(
        can_select_lines: bool,
        selection_count: usize,
        colors: &ThemeColors,
        cx: &mut gpui::Context<Self>,
    ) -> Option<AnyElement> {
        if !can_select_lines || selection_count == 0 {
            return None;
        }
        Some(
            div()
                .px_3()
                .py_1()
                .flex()
                .items_center()
                .gap_2()
                .border_t_1()
                .border_color(colors.border)
                .child(file_action_button(
                    "Stage selected lines",
                    colors,
                    cx,
                    GitronimoApp::stage_selected_diff_lines,
                ))
                .child(file_action_button(
                    "Discard selected lines",
                    colors,
                    cx,
                    GitronimoApp::request_line_discard,
                ))
                .child(
                    div()
                        .px_1()
                        .text_color(colors.text_secondary)
                        .child(format!("{selection_count} line(s) selected")),
                )
                .into_any_element(),
        )
    }

    fn staged_unstaged_tabs(staged_diff: bool, colors: &ThemeColors) -> AnyElement {
        div()
            .flex()
            .p_0p5()
            .bg(colors.raised_background)
            .rounded(px(4.0))
            .child(
                div()
                    .px_2()
                    .py_0p5()
                    .rounded(px(3.0))
                    .text_xs()
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .bg(if staged_diff {
                        colors.raised_background
                    } else {
                        colors.accent
                    })
                    .text_color(if staged_diff {
                        colors.text_secondary
                    } else {
                        colors.panel_background
                    })
                    .child("Unstaged"),
            )
            .child(
                div()
                    .px_2()
                    .py_0p5()
                    .rounded(px(3.0))
                    .text_xs()
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .bg(if staged_diff {
                        colors.accent
                    } else {
                        colors.raised_background
                    })
                    .text_color(if staged_diff {
                        colors.panel_background
                    } else {
                        colors.text_secondary
                    })
                    .child("Staged"),
            )
            .into_any_element()
    }

    fn diff_code_rows(
        &self,
        selectable: bool,
        colors: &ThemeColors,
        cx: &mut gpui::Context<Self>,
    ) -> (usize, usize, usize, Vec<AnyElement>) {
        let mut hunk_count = 0;
        let mut total_additions = 0;
        let mut total_deletions = 0;
        let mut code_rows: Vec<AnyElement> = Vec::new();
        let Some(loaded) = &self.loaded_diff else {
            return (0, 0, 0, code_rows);
        };
        let staged_diff = matches!(&self.selected_diff, Some((_, true)));
        let can_mutate = !self.mutation_in_flight && !loaded.truncated;
        for file in &loaded.diff.files {
            for (hunk_index, hunk) in file.hunks.iter().enumerate() {
                hunk_count += 1;
                for line in &hunk.lines {
                    match line.kind {
                        DiffLineKind::Addition => total_additions += 1,
                        DiffLineKind::Removal => total_deletions += 1,
                        DiffLineKind::Context => {}
                    }
                }
                let header_text = String::from_utf8_lossy(&hunk.header).into_owned();
                let can_hunk = can_mutate;
                code_rows.push(
                    div()
                        .px_2()
                        .py_1()
                        .flex()
                        .items_center()
                        .justify_between()
                        .bg(colors.raised_background)
                        .border_b_1()
                        .border_color(colors.border)
                        .font_family("Monaco")
                        .text_xs()
                        .text_color(colors.text_secondary)
                        .child(div().child(header_text))
                        .when(can_hunk, |row| {
                            row.child(
                                div()
                                    .flex()
                                    .gap_1()
                                    .child(
                                        div()
                                            .id(("discard-chunk", hunk_index))
                                            .px_2()
                                            .py_1()
                                            .rounded(px(3.0))
                                            .text_xs()
                                            .bg(colors.raised_background)
                                            .border_1()
                                            .border_color(colors.border)
                                            .cursor_pointer()
                                            .on_click(cx.listener(move |app, _, _, cx| {
                                                app.request_hunk_discard(hunk_index, cx);
                                            }))
                                            .child("Discard Chunk"),
                                    )
                                    .child(
                                        div()
                                            .id(("stage-chunk", hunk_index))
                                            .px_2()
                                            .py_1()
                                            .rounded(px(3.0))
                                            .text_xs()
                                            .bg(colors.raised_background)
                                            .border_1()
                                            .border_color(colors.border)
                                            .cursor_pointer()
                                            .on_click(cx.listener(move |app, _, _, cx| {
                                                if staged_diff {
                                                    app.unstage_diff_hunk(hunk_index, cx);
                                                } else {
                                                    app.stage_diff_hunk(hunk_index, cx);
                                                }
                                            }))
                                            .child(if staged_diff {
                                                "Unstage Chunk"
                                            } else {
                                                "Stage Chunk"
                                            }),
                                    ),
                            )
                        })
                        .into_any_element(),
                );
                for (line_index, line) in hunk.lines.iter().enumerate() {
                    code_rows.push(
                        self.diff_line_row(hunk_index, line_index, line, selectable, colors, cx),
                    );
                }
            }
        }
        (hunk_count, total_additions, total_deletions, code_rows)
    }

    fn diff_line_row(
        &self,
        hunk_index: usize,
        line_index: usize,
        line: &DiffLine,
        selectable: bool,
        colors: &ThemeColors,
        cx: &mut gpui::Context<Self>,
    ) -> AnyElement {
        let is_change = matches!(line.kind, DiffLineKind::Addition | DiffLineKind::Removal);
        let is_selected = is_change && self.selected_diff_lines.contains(&(hunk_index, line_index));
        let (sign, line_bg, content_color) = match line.kind {
            DiffLineKind::Addition => ("+", colors.added_line, colors.text_primary),
            DiffLineKind::Removal => ("-", colors.removed_line, colors.text_primary),
            DiffLineKind::Context => (" ", colors.panel_background, colors.text_primary),
        };
        let old_gutter = line.old_line.map(|n| n.to_string()).unwrap_or_default();
        let new_gutter = line.new_line.map(|n| n.to_string()).unwrap_or_default();
        let clickable = selectable && is_change;
        let mut content = vec![
            div()
                .w_8()
                .text_right()
                .text_color(colors.text_muted)
                .child(old_gutter)
                .into_any_element(),
            div()
                .w_8()
                .text_right()
                .text_color(colors.text_muted)
                .child(new_gutter)
                .into_any_element(),
            div()
                .w_4()
                .text_color(content_color)
                .child(sign)
                .into_any_element(),
            div()
                .text_color(content_color)
                .child(String::from_utf8_lossy(&line.content).into_owned())
                .into_any_element(),
        ];
        if line.missing_final_newline {
            content.push(
                div()
                    .text_color(colors.text_muted)
                    .child("\\ No newline at end of file")
                    .into_any_element(),
            );
        }
        let styled = |row: gpui::Div| {
            row.px_1()
                .py_0p5()
                .font_family("Monaco")
                .text_xs()
                .flex()
                .items_center()
                .gap_2()
                .bg(if is_selected {
                    colors.selection
                } else {
                    line_bg
                })
        };
        if clickable {
            styled(div())
                .id(SharedString::from(format!(
                    "diff-line-{hunk_index}-{line_index}"
                )))
                .cursor_pointer()
                .on_click(cx.listener(move |app, _, _, cx| {
                    app.toggle_diff_line(hunk_index, line_index, cx);
                }))
                .children(content)
                .into_any_element()
        } else {
            styled(div()).children(content).into_any_element()
        }
    }
}
