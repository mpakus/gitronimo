//! Repositories view: local repository entry points and recent repositories.

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use gpui::{AnyElement, div, prelude::*, px};
use ui_kit::ThemeColors;

use crate::actions::OpenRepository;
use crate::app_state::GitronimoApp;
use crate::views::components::{file_action_button, primary_window_action_button, state_panel};

impl GitronimoApp {
    #[allow(clippy::too_many_lines)]
    pub(crate) fn welcome_view(
        &self,
        colors: &ThemeColors,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let mut repository_rows = Vec::new();
        if self.repositories_grouped {
            let mut groups: BTreeMap<PathBuf, Vec<(usize, PathBuf)>> = BTreeMap::new();
            for (index, path) in self.recents.iter().cloned().enumerate() {
                let folder = path
                    .parent()
                    .map_or_else(|| Path::new("/").to_path_buf(), Path::to_path_buf);
                groups.entry(folder).or_default().push((index, path));
            }
            for (folder, paths) in groups {
                repository_rows.push(
                    div()
                        .mt_2()
                        .text_sm()
                        .text_color(colors.text_secondary)
                        .child(folder.display().to_string())
                        .into_any_element(),
                );
                repository_rows.extend(
                    paths
                        .into_iter()
                        .map(|(index, path)| recent_repository_row(path, index, colors, cx)),
                );
            }
        } else {
            repository_rows.extend(
                self.recents
                    .iter()
                    .cloned()
                    .enumerate()
                    .map(|(index, path)| recent_repository_row(path, index, colors, cx)),
            );
        }
        let recent_rows = if repository_rows.is_empty() {
            state_panel(
                "No saved repositories",
                "Add an existing repository or create a new one to keep it here.",
                colors.text_muted,
                colors,
            )
        } else {
            div()
                .flex()
                .flex_col()
                .gap_2()
                .children(repository_rows)
                .into_any_element()
        };
        div()
            .max_w(px(880.0))
            .flex()
            .flex_col()
            .gap_5()
            .child(
                div()
                    .p_6()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .bg(colors.panel_background)
                    .border_1()
                    .border_color(colors.border)
                    .child(div().text_2xl().child("Repositories"))
                    .child(
                        div().text_color(colors.text_secondary).child(
                            "Keep local repositories organized and open the next project without leaving Gitronimo.",
                        ),
                    )
                    .child(
                        div()
                            .mt_2()
                            .flex()
                            .flex_wrap()
                            .gap_3()
                            .child(primary_window_action_button(
                                "Add existing…",
                                colors,
                                cx,
                                |_, window, cx| {
                                    window.dispatch_action(Box::new(OpenRepository), cx);
                                },
                            ))
                            .child(file_action_button(
                                "Create new…",
                                colors,
                                cx,
                                GitronimoApp::prompt_create_repository,
                            ))
                            .child(file_action_button(
                                "Clone…",
                                colors,
                                cx,
                                GitronimoApp::prompt_clone_repository,
                            )),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(div().text_lg().child("Saved repositories"))
                    .child(file_action_button(
                        if self.repositories_grouped {
                            "Show flat list"
                        } else {
                            "Group by folder"
                        },
                        colors,
                        cx,
                        GitronimoApp::toggle_repositories_grouped,
                    )),
            )
            .child(recent_rows)
    }
}

fn recent_repository_row(
    path: PathBuf,
    index: usize,
    colors: &ThemeColors,
    cx: &mut gpui::Context<GitronimoApp>,
) -> AnyElement {
    let label = path.display().to_string();
    div()
        .id(("recent-repository", index))
        .p_3()
        .bg(colors.raised_background)
        .border_1()
        .border_color(colors.border)
        .cursor_pointer()
        .on_click(cx.listener(move |app, _, window, cx| {
            app.open_recent(path.clone(), window, cx);
        }))
        .child(div().text_lg().child(label))
        .child(
            div()
                .mt_1()
                .text_sm()
                .text_color(colors.text_secondary)
                .child("Local repository · Open this repository"),
        )
        .into_any_element()
}
