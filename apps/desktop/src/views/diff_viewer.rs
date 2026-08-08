//! Diff viewer: rendered unified diff text with hunk-level stage/unstage actions.

use gpui::{div, prelude::*};
use ui_kit::ThemeColors;

use crate::app_state::GitronimoApp;
use crate::views::components::file_action_button;

impl GitronimoApp {
    pub(crate) fn diff_view(
        &self,
        colors: &ThemeColors,
        cx: &mut gpui::Context<Self>,
    ) -> Option<gpui::AnyElement> {
        self.loaded_diff.as_ref().map(|loaded| {
            let mut text = String::new();
            let mut hunk_count = 0;
            for file in &loaded.diff.files {
                for hunk in &file.hunks {
                    hunk_count += 1;
                    text.push_str(&String::from_utf8_lossy(&hunk.header));
                    text.push('\n');
                    for line in &hunk.lines {
                        let prefix = match line.kind {
                            git_domain::DiffLineKind::Context => ' ',
                            git_domain::DiffLineKind::Addition => '+',
                            git_domain::DiffLineKind::Removal => '-',
                        };
                        text.push(prefix);
                        text.push_str(&String::from_utf8_lossy(&line.content));
                        text.push('\n');
                    }
                }
            }
            let can_mutate_hunks = !self.mutation_in_flight
                && !loaded.truncated
                && !loaded.diff.files.iter().any(|file| file.binary)
                && self.selected_diff.is_some();
            let staged_diff = matches!(&self.selected_diff, Some((_, true)));
            div()
                .p_3()
                .flex()
                .flex_col()
                .gap_2()
                .bg(colors.panel_background)
                .border_1()
                .border_color(colors.border)
                .children((can_mutate_hunks && hunk_count > 0).then(|| {
                    div()
                        .flex()
                        .gap_2()
                        .children((0..hunk_count).map(|hunk_index| {
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
                                ))
                        }))
                }))
                .child(if loaded.diff.files.iter().any(|file| file.binary) {
                    "Binary file changed".to_owned()
                } else {
                    text
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
}
