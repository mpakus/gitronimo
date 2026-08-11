//! Tree view: browse a commit's tree, drill into directories, read blobs,
//! and export a selected file to disk.

use gpui::{ClickEvent, SharedString, div, prelude::*, px};
use ui_kit::ThemeColors;

use git_domain::{TreeEntryKind, WorktreeRepository};

use crate::app_state::GitronimoApp;
use crate::views::components::{
    centered_empty_state, file_action_button, two_pane_view, view_panel_header,
};

impl GitronimoApp {
    #[allow(clippy::too_many_lines)]
    pub(crate) fn tree_view(
        &self,
        repository: &WorktreeRepository,
        colors: &ThemeColors,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let list_colors = *colors;
        let current = repository.clone();
        let mut rows = Vec::new();
        for entry in &self.tree {
            let name = String::from_utf8_lossy(&entry.name.0).to_string();
            let kind_label = match entry.kind {
                TreeEntryKind::Tree => "tree",
                TreeEntryKind::Blob => "blob",
                TreeEntryKind::Commit => "submodule",
            };
            let oid = String::from_utf8_lossy(&entry.oid).to_string();
            let repository = current.clone();
            let snapshot = entry.clone();
            rows.push(
                div()
                    .id(SharedString::from(format!("tree-{kind_label}-{name}")))
                    .h(px(28.0))
                    .px_3()
                    .flex()
                    .items_center()
                    .border_b_1()
                    .border_color(list_colors.separator)
                    .bg(list_colors.panel_background)
                    .cursor_pointer()
                    .on_click(cx.listener(move |app, _: &ClickEvent, _, cx| {
                        app.select_tree_entry(&snapshot, repository.clone(), cx);
                    }))
                    .child(format!("{kind_label}  {name}  {oid}"))
                    .into_any_element(),
            );
        }
        let list = if rows.is_empty() {
            centered_empty_state(
                "Empty tree",
                "Browse another commit or navigate up a level.",
                colors,
            )
        } else {
            div().flex().flex_col().children(rows).into_any_element()
        };
        let detail = self.tree_blob.as_ref().map_or_else(
            || {
                centered_empty_state(
                    "No blob selected",
                    "Select a file entry to preview its contents.",
                    colors,
                )
            },
            |blob| {
                let text = String::from_utf8_lossy(blob).into_owned();
                let preview = text
                    .char_indices()
                    .nth(4000)
                    .map_or(text.as_str(), |(index, _)| &text[..index]);
                div()
                    .p_4()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .font_family("Monaco")
                    .text_xs()
                    .child(if text.len() > preview.len() {
                        format!("{} bytes (preview below)\n{}", blob.len(), preview)
                    } else {
                        text
                    })
                    .into_any_element()
            },
        );
        let header_actions = div()
            .flex()
            .gap_1()
            .child(file_action_button(
                "Browse commit…",
                colors,
                cx,
                GitronimoApp::prompt_browse_tree,
            ))
            .children((!self.tree_path.is_empty()).then(|| {
                let repository = current.clone();
                file_action_button("Up one level", colors, cx, move |app, cx| {
                    app.back_tree_level(repository.clone(), cx);
                })
            }))
            .child(file_action_button("Export file…", colors, cx, |_, cx| {
                GitronimoApp::export_selected_blob(cx);
            }))
            .into_any_element();
        let summary = div()
            .p_4()
            .flex()
            .flex_col()
            .gap_1()
            .child(
                div()
                    .text_sm()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .child(format!("Commit: {}", self.tree_oid)),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(colors.text_muted)
                    .child(format!("Path: {}", self.tree_path_label())),
            )
            .into_any_element();
        two_pane_view(
            view_panel_header("Browse Tree", colors, Some(header_actions)),
            list,
            div()
                .flex()
                .flex_col()
                .h_full()
                .child(summary)
                .child(detail)
                .into_any_element(),
            colors,
        )
    }
}
