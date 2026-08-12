//! Commit Detail view: full metadata, changed-file list with a read-only diff,
//! and an optional tree-browsing mode at the selected commit.

use gpui::{AnyElement, ClickEvent, SharedString, div, prelude::*, px};
use ui_kit::ThemeColors;

use git_domain::{DiffLineKind, TreeEntryKind, WorktreeRepository};

use crate::app_state::{GitronimoApp, HistoryDetailMode, RepositoryView};
use crate::views::components::file_action_button;

impl GitronimoApp {
    pub(crate) fn commit_detail_view(
        &self,
        repository: &WorktreeRepository,
        colors: &ThemeColors,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let Some(commit) = self
            .selected_history
            .and_then(|index| self.history.get(index))
        else {
            return Self::empty_commit_detail(colors).into_any_element();
        };
        let current = repository.clone();
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(div().text_xl().child("Commit detail"))
            .child(div().child(format!("OID: {}", commit.oid)))
            .child(div().child(format!(
                "Author: {}",
                String::from_utf8_lossy(&commit.author.name)
            )))
            .child(div().child(format!("Date: {}", commit.author.timestamp)))
            .child(div().child(format!(
                "Subject: {}",
                String::from_utf8_lossy(&commit.subject)
            )))
            .child(
                div()
                    .child(format!("Body: {}", String::from_utf8_lossy(&commit.body)))
                    .when(commit.body.is_empty(), |this| {
                        this.child(div().text_color(colors.text_muted).child("(empty)"))
                    }),
            )
            .child(div().child(format!("Parents: {}", commit.parents.join(" "))))
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(file_action_button("Working Copy", colors, cx, |app, cx| {
                        app.navigate_to(RepositoryView::WorkingCopy, cx);
                    }))
                    .child(file_action_button("History", colors, cx, {
                        let repository = current.clone();
                        move |app, cx| app.show_history(repository.clone(), cx)
                    })),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(self.detail_mode_button(
                        HistoryDetailMode::Changeset,
                        "Changeset",
                        colors,
                        cx,
                        repository,
                    ))
                    .child(self.detail_mode_button(
                        HistoryDetailMode::Tree,
                        "Tree",
                        colors,
                        cx,
                        repository,
                    )),
            )
            .when(
                self.history_detail_mode == HistoryDetailMode::Changeset,
                |this| this.child(self.changeset_panel(colors)),
            )
            .when(
                self.history_detail_mode == HistoryDetailMode::Tree,
                |this| this.child(self.tree_panel(repository, colors, cx)),
            )
            .into_any_element()
    }

    fn empty_commit_detail(colors: &ThemeColors) -> AnyElement {
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(div().text_xl().child("Commit detail"))
            .child(
                div()
                    .text_color(colors.text_muted)
                    .child("No commit selected."),
            )
            .into_any_element()
    }

    pub(crate) fn detail_mode_button(
        &self,
        mode: HistoryDetailMode,
        label: &'static str,
        colors: &ThemeColors,
        cx: &mut gpui::Context<Self>,
        repository: &WorktreeRepository,
    ) -> AnyElement {
        let active = self.history_detail_mode == mode;
        let repository = repository.clone();
        div()
            .id(gpui::ElementId::Name(format!("detail-tab:{label}").into()))
            .px_2()
            .py_1()
            .bg(if active {
                colors.raised_background
            } else {
                colors.panel_background
            })
            .border_1()
            .border_color(if active { colors.accent } else { colors.border })
            .cursor_pointer()
            .on_click(cx.listener(move |app, _, _, cx| {
                app.toggle_history_detail_mode(mode, repository.clone(), cx);
            }))
            .child(label)
            .into_any_element()
    }

    pub(crate) fn changeset_panel(&self, colors: &ThemeColors) -> AnyElement {
        let changed: Vec<String> = self
            .history_paths
            .iter()
            .map(|path| String::from_utf8_lossy(&path.0).to_string())
            .collect();
        let file_list = div()
            .w(px(240.0))
            .h_full()
            .border_r_1()
            .border_color(colors.border)
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(28.0))
                    .px_2()
                    .flex()
                    .items_center()
                    .border_b_1()
                    .border_color(colors.separator)
                    .text_xs()
                    .text_color(colors.text_muted)
                    .child(format!("{} changed file(s)", changed.len())),
            )
            .children(changed.iter().enumerate().map(|(index, path)| {
                div()
                    .id(SharedString::from(format!("changeset-file-{index}")))
                    .h(px(22.0))
                    .px_2()
                    .flex()
                    .items_center()
                    .border_b_1()
                    .border_color(colors.separator)
                    .text_xs()
                    .text_color(colors.text_secondary)
                    .child(path.clone())
                    .into_any_element()
            }));
        div()
            .flex()
            .h_full()
            .overflow_hidden()
            .child(file_list)
            .child(
                div()
                    .flex_1()
                    .overflow_hidden()
                    .child(self.readonly_diff(colors)),
            )
            .into_any_element()
    }

    fn readonly_diff(&self, colors: &ThemeColors) -> AnyElement {
        let Some(loaded) = &self.history_diff else {
            return div()
                .text_color(colors.text_muted)
                .child("Loading diff…")
                .into_any_element();
        };
        let mut rows: Vec<AnyElement> = Vec::new();
        if loaded.diff.files.is_empty() {
            rows.push(
                div()
                    .text_color(colors.text_muted)
                    .child("No file changes in this commit.")
                    .into_any_element(),
            );
        }
        for file in &loaded.diff.files {
            let path = file
                .new_path
                .as_ref()
                .or(file.old_path.as_ref())
                .map(|path| String::from_utf8_lossy(&path.0).into_owned())
                .unwrap_or_default();
            rows.push(
                div()
                    .font_family("Monaco")
                    .text_color(colors.text_secondary)
                    .child(format!("File: {path}"))
                    .into_any_element(),
            );
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
                            .child(format!("{sign} {}", String::from_utf8_lossy(&line.content)))
                            .into_any_element(),
                    );
                }
            }
        }
        div()
            .p_2()
            .flex()
            .flex_col()
            .bg(colors.panel_background)
            .border_1()
            .border_color(colors.border)
            .children(rows)
            .into_any_element()
    }

    pub(crate) fn tree_panel(
        &self,
        repository: &WorktreeRepository,
        colors: &ThemeColors,
        cx: &mut gpui::Context<Self>,
    ) -> AnyElement {
        let current = repository.clone();
        let mut content = vec![
            div()
                .child(div().child(format!("Commit: {}", self.tree_oid)))
                .child(div().child(format!("Path: {}", self.tree_path_label())))
                .into_any_element(),
        ];
        if !self.tree_path.is_empty() {
            content.push(
                div()
                    .child(file_action_button("Up one level", colors, cx, {
                        let repository = current.clone();
                        move |app, cx| app.back_tree_level(repository.clone(), cx)
                    }))
                    .into_any_element(),
            );
        }
        if self.tree.is_empty() {
            content.push(
                div()
                    .text_color(colors.text_muted)
                    .child("Loading tree…")
                    .into_any_element(),
            );
        }
        content.extend(self.tree.iter().map(|entry| {
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
                .id(SharedString::from(format!(
                    "detail-tree-{kind_label}-{name}"
                )))
                .px_2()
                .h(px(24.0))
                .flex()
                .items_center()
                .bg(colors.panel_background)
                .border_b_1()
                .border_color(colors.border)
                .cursor_pointer()
                .on_click(cx.listener(move |app, _: &ClickEvent, _, cx| {
                    app.select_tree_entry(&snapshot, repository.clone(), cx);
                }))
                .child(format!("{kind_label}  {name}  {oid}"))
                .into_any_element()
        }));
        if let Some(blob) = &self.tree_blob {
            let text = String::from_utf8_lossy(blob).into_owned();
            content.push(
                div()
                    .p_2()
                    .border_1()
                    .border_color(colors.border)
                    .font_family("Monaco")
                    .child(text)
                    .into_any_element(),
            );
        }
        div()
            .flex()
            .flex_col()
            .gap_1()
            .children(content)
            .into_any_element()
    }
}
