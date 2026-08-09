//! Compare view: a read-only unified diff between two refs.

use gpui::{div, prelude::*};
use ui_kit::ThemeColors;

use git_domain::{DiffLineKind, WorktreeRepository};

use crate::app_state::{GitronimoApp, RepositoryView};
use crate::views::components::file_action_button;

impl GitronimoApp {
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
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(div().text_xl().child("Compare refs"))
            .child(div().child(format!(
                "{}…{}  ({} file(s))",
                self.compare_left, self.compare_right, file_count
            )))
            .child(file_action_button("Working Copy", colors, cx, |app, cx| {
                app.navigate_to(RepositoryView::WorkingCopy, cx);
            }))
            .child(file_action_button(
                "Compare another pair…",
                colors,
                cx,
                |_, cx| GitronimoApp::prompt_compare_refs(cx),
            ))
            .children(rows)
            .children(truncated.then(|| div().child("Diff truncated to the display limit.")))
    }
}
