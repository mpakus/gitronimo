//! Git LFS status view.

use gpui::{div, prelude::*, px};
use ui_kit::ThemeColors;

use crate::app_state::{GitronimoApp, RepositoryView, ShellState};
use crate::views::components::{
    centered_empty_state, file_action_button, two_pane_view, view_panel_header,
};

impl GitronimoApp {
    pub(crate) fn lfs_view(
        &self,
        colors: &ThemeColors,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let list_colors = *colors;
        let mut rows = Vec::new();
        for (index, entry) in self.lfs.iter().enumerate() {
            let path = String::from_utf8_lossy(&entry.path.0).into_owned();
            rows.push(
                div()
                    .id(index)
                    .h(px(28.0))
                    .px_3()
                    .flex()
                    .items_center()
                    .gap_3()
                    .border_b_1()
                    .border_color(list_colors.separator)
                    .bg(list_colors.panel_background)
                    .child(
                        div()
                            .font_family("Monaco")
                            .text_xs()
                            .text_color(colors.accent)
                            .child(format!(
                                "{}{}",
                                char::from(entry.index_status),
                                char::from(entry.worktree_status)
                            )),
                    )
                    .child(path)
                    .into_any_element(),
            );
        }
        let list = if rows.is_empty() {
            centered_empty_state(
                "No changed LFS files",
                "Git LFS paths with local changes appear here.",
                colors,
            )
        } else {
            div().flex().flex_col().children(rows).into_any_element()
        };
        let detail = div()
            .flex()
            .flex_col()
            .gap_3()
            .p_4()
            .child(
                div()
                    .text_sm()
                    .text_color(colors.text_secondary)
                    .child("Changed LFS paths reported by the installed Git LFS client."),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(colors.text_muted)
                    .child(
                        "Fetch downloads objects. Pull downloads them and checks them out into the working copy.",
                    ),
            )
            .child(file_action_button("Fetch LFS", colors, cx, |app, cx| {
                app.fetch_lfs(cx);
            }))
            .child(file_action_button("Pull LFS", colors, cx, |app, cx| {
                app.pull_lfs(cx);
            }))
            .child(file_action_button("Working Copy", colors, cx, |app, cx| {
                app.navigate_to(RepositoryView::WorkingCopy, cx);
            }))
            .into_any_element();
        let header_actions = div()
            .flex()
            .gap_1()
            .child(file_action_button("Refresh", colors, cx, |app, cx| {
                if let ShellState::Repository(repository) = &app.state {
                    app.load_lfs(repository.clone(), cx);
                }
            }))
            .into_any_element();
        two_pane_view(
            view_panel_header("Git LFS", colors, Some(header_actions)),
            list,
            detail,
            colors,
        )
    }
}
