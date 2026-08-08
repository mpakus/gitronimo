//! Diff viewer: per-line rendered diff rows with line selection for partial
//! staging or discarding, plus hunk-level stage/unstage actions.

use gpui::{AnyElement, SharedString, div, prelude::*};
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
            let (hunk_count, code_rows) = self.diff_code_rows(can_select_lines, colors, cx);
            div()
                .p_3()
                .flex()
                .flex_col()
                .gap_2()
                .bg(colors.panel_background)
                .border_1()
                .border_color(colors.border)
                .children(
                    (can_mutate_hunks && hunk_count > 0)
                        .then(|| Self::hunk_controls_row(hunk_count, staged_diff, colors, cx)),
                )
                .children((can_select_lines && selection_count > 0).then(|| {
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
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
                        .into_any_element()
                }))
                .child(if is_binary {
                    div().child("Binary file changed").into_any_element()
                } else {
                    div().children(code_rows).into_any_element()
                })
                .children(loaded.truncated.then(|| {
                    div().child("Diff truncated.").child(file_action_button(
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

    fn hunk_controls_row(
        hunk_count: usize,
        staged_diff: bool,
        colors: &ThemeColors,
        cx: &mut gpui::Context<Self>,
    ) -> AnyElement {
        div()
            .flex()
            .flex_col()
            .gap_1()
            .children((0..hunk_count).map(|hunk_index| {
                let mut row = div().flex().gap_1().child(
                    div()
                        .id(("diff-hunk", hunk_index))
                        .px_2()
                        .py_1()
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
                        .child(format!(
                            "{} hunk {}",
                            if staged_diff { "Unstage" } else { "Stage" },
                            hunk_index + 1
                        )),
                );
                if !staged_diff {
                    row = row.child(
                        div()
                            .id(("discard-hunk", hunk_index))
                            .px_2()
                            .py_1()
                            .bg(colors.raised_background)
                            .border_1()
                            .border_color(colors.border)
                            .cursor_pointer()
                            .on_click(cx.listener(move |app, _, _, cx| {
                                app.request_hunk_discard(hunk_index, cx);
                            }))
                            .child(format!("Discard hunk {}", hunk_index + 1)),
                    );
                }
                row.into_any_element()
            }))
            .into_any_element()
    }

    fn diff_code_rows(
        &self,
        selectable: bool,
        colors: &ThemeColors,
        cx: &mut gpui::Context<Self>,
    ) -> (usize, Vec<AnyElement>) {
        let mut hunk_count = 0;
        let mut code_rows: Vec<AnyElement> = Vec::new();
        let Some(loaded) = &self.loaded_diff else {
            return (0, code_rows);
        };
        for file in &loaded.diff.files {
            for (hunk_index, hunk) in file.hunks.iter().enumerate() {
                hunk_count += 1;
                code_rows.push(
                    div()
                        .px_2()
                        .py_1()
                        .font_family("Monaco")
                        .text_color(colors.text_secondary)
                        .child(String::from_utf8_lossy(&hunk.header).into_owned())
                        .into_any_element(),
                );
                for (line_index, line) in hunk.lines.iter().enumerate() {
                    code_rows.push(
                        self.diff_line_row(hunk_index, line_index, line, selectable, colors, cx),
                    );
                }
            }
        }
        (hunk_count, code_rows)
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
        let (sign, content_color) = match line.kind {
            DiffLineKind::Addition => ("+", colors.added_line),
            DiffLineKind::Removal => ("-", colors.removed_line),
            DiffLineKind::Context => (" ", colors.text_primary),
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
                .flex()
                .items_center()
                .gap_2()
                .when(is_selected, |row| row.bg(colors.selection))
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
