//! Git LFS status view.

use gpui::{div, prelude::*};
use ui_kit::ThemeColors;

use crate::app_state::{GitronimoApp, RepositoryView, ShellState};
use crate::views::components::file_action_button;

impl GitronimoApp {
    pub(crate) fn lfs_view(
        &self,
        colors: &ThemeColors,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(div().text_xl().child("Git LFS status"))
            .child(
                div()
                    .text_color(colors.text_secondary)
                    .child("Changed LFS paths reported by the installed Git LFS client."),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(file_action_button("Working Copy", colors, cx, |app, cx| {
                        app.navigate_to(RepositoryView::WorkingCopy, cx);
                    }))
                    .child(file_action_button("Refresh", colors, cx, |app, cx| {
                        if let ShellState::Repository(repository) = &app.state {
                            app.load_lfs(repository.clone(), cx);
                        }
                    })),
            )
            .children(if self.lfs.is_empty() {
                Some(
                    div()
                        .p_3()
                        .bg(colors.panel_background)
                        .border_1()
                        .border_color(colors.border)
                        .text_color(colors.text_muted)
                        .child("No changed Git LFS files."),
                )
            } else {
                None
            })
            .children(self.lfs.iter().map(|entry| {
                let path = String::from_utf8_lossy(&entry.path.0).into_owned();
                div()
                    .p_2()
                    .flex()
                    .items_center()
                    .gap_3()
                    .bg(colors.panel_background)
                    .border_1()
                    .border_color(colors.border)
                    .child(
                        div()
                            .font_family("Monaco")
                            .text_color(colors.accent)
                            .child(format!(
                                "{}{}",
                                char::from(entry.index_status),
                                char::from(entry.worktree_status)
                            )),
                    )
                    .child(path)
            }))
    }
}
