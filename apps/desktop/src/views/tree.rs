//! Tree view: browse a commit's tree, drill into directories, read blobs,
//! and export a selected file to disk.

use gpui::{ClickEvent, SharedString, div, prelude::*, px};
use ui_kit::ThemeColors;

use git_domain::{TreeEntryKind, WorktreeRepository};

use crate::app_state::{GitronimoApp, RepositoryView};
use crate::views::components::file_action_button;

impl GitronimoApp {
    pub(crate) fn tree_view(
        &self,
        repository: &WorktreeRepository,
        colors: &ThemeColors,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let list_colors = *colors;
        let current = repository.clone();
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(div().text_xl().child("Browse tree"))
            .child(div().child(format!("Commit: {}", self.tree_oid)))
            .child(div().child(format!("Path: {}", self.tree_path_label())))
            .child(file_action_button("Working Copy", colors, cx, |app, cx| {
                app.navigate_to(RepositoryView::WorkingCopy, cx);
            }))
            .child(file_action_button(
                "Browse another commit…",
                colors,
                cx,
                |_, cx| GitronimoApp::prompt_browse_tree(cx),
            ))
            .children((!self.tree_path.is_empty()).then(|| {
                let repository = current.clone();
                file_action_button("Up one level", colors, cx, move |app, cx| {
                    app.back_tree_level(repository.clone(), cx);
                })
            }))
            .children(self.tree.iter().map(|entry| {
                let name = String::from_utf8_lossy(&entry.name.0).to_string();
                let kind_label = match entry.kind {
                    TreeEntryKind::Tree => "tree",
                    TreeEntryKind::Blob => "blob",
                    TreeEntryKind::Commit => "submodule",
                };
                let oid = String::from_utf8_lossy(&entry.oid).to_string();
                let repository = current.clone();
                let snapshot = entry.clone();
                div()
                    .id(SharedString::from(format!("tree-{kind_label}-{name}")))
                    .px_2()
                    .h(px(24.0))
                    .flex()
                    .items_center()
                    .bg(list_colors.panel_background)
                    .border_b_1()
                    .border_color(list_colors.border)
                    .cursor_pointer()
                    .on_click(cx.listener(move |app, _: &ClickEvent, _, cx| {
                        app.select_tree_entry(&snapshot, repository.clone(), cx);
                    }))
                    .child(format!("{kind_label}  {name}  {oid}"))
            }))
            .children(self.tree_blob.as_ref().map(|blob| {
                let text = String::from_utf8_lossy(blob).into_owned();
                let preview = text
                    .char_indices()
                    .nth(4000)
                    .map_or(text.as_str(), |(index, _)| &text[..index]);
                div()
                    .p_2()
                    .border_1()
                    .border_color(colors.border)
                    .font_family("Monaco")
                    .child(if text.len() > preview.len() {
                        format!("{} bytes (preview below)\n{}", blob.len(), preview)
                    } else {
                        text
                    })
            }))
            .child(file_action_button(
                "Export file at revision…",
                colors,
                cx,
                |_, cx| GitronimoApp::export_selected_blob(cx),
            ))
    }
}
