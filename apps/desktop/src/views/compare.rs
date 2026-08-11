//! Compare view: a read-only unified diff between two refs.

use gpui::{div, prelude::*};
use ui_kit::ThemeColors;

use git_domain::{DiffLineKind, WorktreeRepository};

use crate::app_state::GitronimoApp;
use crate::views::components::{
    centered_empty_state, file_action_button, two_pane_view, view_panel_header,
};

impl GitronimoApp {
    #[allow(clippy::too_many_lines)]
    pub(crate) fn compare_view(
        &self,
        _repository: &WorktreeRepository,
        colors: &ThemeColors,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let rows = self
            .compare_diff
            .as_ref()
            .map(|loaded| {
                let mut rows = Vec::new();
                for file in &loaded.diff.files {
                    for hunk in &file.hunks {
                        rows.push(
                            div()
                                .font_family("Monaco")
                                .text_color(colors.text_secondary)
                                .child(String::from_utf8_lossy(&hunk.header).into_owned())
                                .into_any_element(),
                        );
                        for line in &hunk.lines {
                            let (sign, color) = match line.kind {
                                DiffLineKind::Addition => ("+", colors.added_line),
                                DiffLineKind::Removal => ("-", colors.removed_line),
                                DiffLineKind::Context => (" ", colors.text_primary),
                            };
                            rows.push(
                                div()
                                    .font_family("Monaco")
                                    .text_color(color)
                                    .child(format!(
                                        "{sign} {}",
                                        String::from_utf8_lossy(&line.content)
                                    ))
                                    .into_any_element(),
                            );
                        }
                    }
                }
                rows
            })
            .unwrap_or_default();
        let file_count = self
            .compare_diff
            .as_ref()
            .map_or(0, |loaded| loaded.diff.files.len());
        let truncated = self
            .compare_diff
            .as_ref()
            .is_some_and(|loaded| loaded.truncated);
        let list = if self.compare_left.is_empty() && self.compare_right.is_empty() {
            centered_empty_state(
                "No comparison loaded",
                "Compare two refs to inspect their diff.",
                colors,
            )
        } else {
            div()
                .p_4()
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    div()
                        .text_sm()
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .child(format!(
                            "{}…{}  ({} file(s))",
                            self.compare_left, self.compare_right, file_count
                        )),
                )
                .children(truncated.then(|| {
                    div()
                        .text_xs()
                        .text_color(colors.warning)
                        .child("Diff truncated to the display limit.")
                }))
                .into_any_element()
        };
        let detail = if rows.is_empty() {
            centered_empty_state(
                "No diff output",
                "Choose another ref pair or refresh the comparison.",
                colors,
            )
        } else {
            div()
                .p_2()
                .flex()
                .flex_col()
                .gap_0()
                .children(rows)
                .into_any_element()
        };
        let header_actions = div()
            .flex()
            .gap_1()
            .child(file_action_button(
                "Compare refs…",
                colors,
                cx,
                |_, cx| GitronimoApp::prompt_compare_refs(cx),
            ))
            .into_any_element();
        two_pane_view(
            view_panel_header("Compare", colors, Some(header_actions)),
            list,
            detail,
            colors,
        )
    }
}
