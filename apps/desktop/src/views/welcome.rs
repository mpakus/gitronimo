//! Repositories view: grouped sidebar list plus a detail panel for the selection.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use gpui::{AnyElement, div, prelude::*, px};
use ui_kit::ThemeColors;

use crate::actions::OpenRepository;
use crate::app_state::{GitronimoApp, WelcomeRepoSnapshot, WelcomeShellView};
use crate::views::components::{
    centered_empty_state, detail_row, detail_section, file_action_button,
    primary_window_action_button, welcome_rail_tab,
};
use crate::views::single_line_input::single_line_input_shell;

impl GitronimoApp {
    pub(crate) fn welcome_vertical_rail(
        &self,
        colors: &ThemeColors,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        div()
            .w(px(72.0))
            .h_full()
            .flex()
            .flex_col()
            .bg(colors.sidebar_background)
            .border_r_1()
            .border_color(colors.border)
            .child(welcome_rail_tab(
                "Services",
                "\u{2699}",
                self.welcome_shell_view == WelcomeShellView::Services,
                colors,
                cx,
                |app, cx| app.set_welcome_shell_view(WelcomeShellView::Services, cx),
            ))
            .child(welcome_rail_tab(
                "Bookmarks",
                "\u{2605}",
                self.welcome_shell_view == WelcomeShellView::Repositories,
                colors,
                cx,
                |app, cx| app.set_welcome_shell_view(WelcomeShellView::Repositories, cx),
            ))
            .child(welcome_rail_tab(
                "Workflow",
                "\u{21BB}",
                self.welcome_shell_view == WelcomeShellView::Workflow,
                colors,
                cx,
                |app, cx| app.set_welcome_shell_view(WelcomeShellView::Workflow, cx),
            ))
    }

    #[allow(clippy::unused_self)]
    pub(crate) fn welcome_view(
        &self,
        colors: &ThemeColors,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        if self.welcome_shell_view == WelcomeShellView::Services {
            return div()
                .flex_1()
                .h_full()
                .overflow_hidden()
                .bg(colors.window_background)
                .child(self.services_view(colors, cx))
                .into_any_element();
        }
        if self.welcome_shell_view == WelcomeShellView::Workflow {
            return welcome_workflow_hub(self, colors, cx).into_any_element();
        }
        div()
            .flex_1()
            .flex()
            .flex_col()
            .bg(colors.window_background)
            .child(
                div()
                    .flex_1()
                    .overflow_hidden()
                    .child(welcome_detail_content(self, colors, cx)),
            )
            .child(welcome_action_bar(colors, cx))
            .into_any_element()
    }
}

fn welcome_detail_content(
    app: &GitronimoApp,
    colors: &ThemeColors,
    cx: &mut gpui::Context<GitronimoApp>,
) -> AnyElement {
    let Some(index) = app.selected_recent else {
        return welcome_empty_state(colors);
    };
    let Some(path) = app.recents.get(index) else {
        return welcome_empty_state(colors);
    };
    welcome_repo_detail(path, index, app.welcome_snapshot.as_ref(), app, colors, cx)
}

fn welcome_empty_state(colors: &ThemeColors) -> AnyElement {
    centered_empty_state(
        "Select a repository",
        "Choose a repository from the sidebar, or add one below. You can also drop a folder anywhere on the window.",
        colors,
    )
}

#[allow(clippy::too_many_lines)]
fn welcome_repo_detail(
    path: &Path,
    index: usize,
    snapshot: Option<&WelcomeRepoSnapshot>,
    app: &GitronimoApp,
    colors: &ThemeColors,
    cx: &mut gpui::Context<GitronimoApp>,
) -> AnyElement {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("Repository")
        .to_owned();
    let location = display_path(path);
    let last_opened = format_last_opened(index, snapshot.and_then(|data| data.last_modified));
    let branch = snapshot
        .and_then(|data| data.branch.clone())
        .unwrap_or_else(|| "Loading…".into());
    let changed = snapshot.and_then(|data| data.changed_files);
    let status = changed.map_or_else(
        || "Loading…".into(),
        |count| {
            if count == 0 {
                "No changed files".into()
            } else if count == 1 {
                "1 changed file".into()
            } else {
                format!("{count} changed files")
            }
        },
    );
    let remote = snapshot
        .and_then(|data| data.remote_url.clone())
        .unwrap_or_else(|| "Not configured".into());
    let author = snapshot.and_then(|data| match (&data.author_name, &data.author_email) {
        (Some(name), Some(email)) => Some(format!("{name} <{email}>")),
        (Some(name), None) => Some(name.clone()),
        _ => None,
    });
    let author_initial = snapshot
        .and_then(|data| data.author_name.as_ref())
        .and_then(|name| name.chars().next())
        .map_or_else(|| "?".to_owned(), |ch| ch.to_uppercase().to_string());
    let last_commit = snapshot
        .and_then(|data| data.last_commit_subject.clone())
        .or_else(|| app.last_commit_summary.clone())
        .unwrap_or_else(|| "No commits yet".into());
    let upstream = snapshot
        .and_then(|data| data.upstream.clone())
        .unwrap_or_else(|| "Not configured".into());
    let tracking = snapshot.map_or_else(
        || "Loading…".into(),
        |data| format_upstream_tracking(data.ahead, data.behind),
    );
    let availability = snapshot.map_or("Checking availability…", |data| {
        if data.available {
            ""
        } else {
            "Repository folder is unavailable."
        }
    });

    div()
        .size_full()
        .overflow_hidden()
        .p_8()
        .flex()
        .flex_col()
        .gap_6()
        .child(
            div()
                .flex()
                .flex_col()
                .gap_3()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_3()
                        .child(
                            div()
                                .w(px(36.0))
                                .h(px(36.0))
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded_full()
                                .bg(colors.accent)
                                .text_sm()
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .text_color(colors.panel_background)
                                .child(author_initial),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_0p5()
                                .child(
                                    div()
                                        .text_xl()
                                        .font_weight(gpui::FontWeight::SEMIBOLD)
                                        .text_color(colors.text_primary)
                                        .child(name),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(colors.text_muted)
                                        .child(last_commit),
                                ),
                        ),
                )
                .child(
                    div()
                        .id("repo-description-field")
                        .child(single_line_input_shell(
                            app.repo_description_input.clone(),
                            colors,
                            false,
                        )),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .text_xs()
                                .text_color(colors.text_muted)
                                .child("REPOSITORY"),
                        )
                        .child(primary_window_action_button(
                            "Open",
                            true,
                            colors,
                            cx,
                            |app, window, cx| {
                                app.open_selected_recent(window, cx);
                            },
                        ))
                        .child(file_action_button("Delete", colors, cx, |app, cx| {
                            app.confirm_remove_selected_recent(cx);
                        })),
                )
                .when(!availability.is_empty(), |panel| {
                    panel.child(
                        div()
                            .text_sm()
                            .text_color(colors.warning)
                            .child(availability),
                    )
                }),
        )
        .child(detail_section("Repository", colors))
        .child(
            div()
                .flex()
                .flex_col()
                .gap_0()
                .child(detail_row("Location", &location, colors))
                .child(detail_row("Last Opened", &last_opened, colors)),
        )
        .child(detail_section("Working Copy", colors))
        .child(
            div()
                .flex()
                .flex_col()
                .gap_0()
                .child(detail_row("Current Branch", &branch, colors))
                .child(detail_row("Upstream", &upstream, colors))
                .child(detail_row("Tracking", &tracking, colors))
                .child(detail_row("Status", &status, colors)),
        )
        .child(detail_section("Remotes", colors))
        .child(
            div()
                .flex()
                .flex_col()
                .gap_0()
                .child(detail_row("origin", &remote, colors)),
        )
        .when(author.is_some(), |panel| {
            panel
                .child(detail_section("Committer Identity", colors))
                .child(div().flex().flex_col().gap_0().child(detail_row(
                    "Identity",
                    author.as_deref().unwrap_or_default(),
                    colors,
                )))
        })
        .into_any_element()
}

fn welcome_action_bar(colors: &ThemeColors, cx: &mut gpui::Context<GitronimoApp>) -> AnyElement {
    div()
        .px_4()
        .py_2()
        .flex()
        .items_center()
        .gap_2()
        .bg(colors.panel_background)
        .border_t_1()
        .border_color(colors.border)
        .child(primary_window_action_button(
            "Add",
            true,
            colors,
            cx,
            |_, window, cx| {
                window.dispatch_action(Box::new(OpenRepository), cx);
            },
        ))
        .child(file_action_button("Create", colors, cx, |app, cx| {
            app.prompt_create_repository(cx);
        }))
        .child(file_action_button("Clone", colors, cx, |app, cx| {
            app.prompt_clone_repository(cx);
        }))
        .into_any_element()
}

fn welcome_workflow_hub(
    app: &GitronimoApp,
    colors: &ThemeColors,
    cx: &mut gpui::Context<GitronimoApp>,
) -> AnyElement {
    let last_action = match app.last_action {
        Some(crate::app_state::LastAction::Refresh) => "Last action: refreshed working copy",
        None => "Open a repository to start working",
    };
    let last_commit = app
        .last_commit_summary
        .as_deref()
        .unwrap_or("No recent commit loaded");
    div()
        .flex_1()
        .h_full()
        .overflow_hidden()
        .bg(colors.window_background)
        .p_8()
        .flex()
        .flex_col()
        .gap_6()
        .child(
            div()
                .text_xl()
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(colors.text_primary)
                .child("Workflow"),
        )
        .child(detail_section("Recent activity", colors))
        .child(detail_row("Summary", last_action, colors))
        .child(detail_row("Last commit", last_commit, colors))
        .child(
            div()
                .flex()
                .gap_2()
                .child(primary_window_action_button(
                    "Open Working Copy",
                    matches!(app.state, crate::app_state::ShellState::Repository(_)),
                    colors,
                    cx,
                    |app, _, cx| {
                        app.navigate_to(crate::app_state::RepositoryView::WorkingCopy, cx);
                    },
                ))
                .child(file_action_button(
                    "Open repository…",
                    colors,
                    cx,
                    |_, cx| {
                        cx.notify();
                    },
                )),
        )
        .into_any_element()
}

fn format_upstream_tracking(ahead: u32, behind: u32) -> String {
    if ahead == 0 && behind == 0 {
        "Up to date".into()
    } else {
        let mut parts = Vec::new();
        if ahead > 0 {
            parts.push(format!("{ahead} ahead"));
        }
        if behind > 0 {
            parts.push(format!("{behind} behind"));
        }
        parts.join(", ")
    }
}

fn display_path(path: &Path) -> String {
    if let Ok(home) = std::env::var("HOME") {
        let home = PathBuf::from(home);
        if let Ok(stripped) = path.strip_prefix(&home) {
            return format!("~/{}", stripped.display());
        }
    }
    path.display().to_string()
}

fn format_last_opened(_index: usize, modified: Option<SystemTime>) -> String {
    let Some(modified) = modified else {
        return "Unknown".into();
    };
    let Ok(age) = SystemTime::now().duration_since(modified) else {
        return "Unknown".into();
    };
    let seconds = age.as_secs();
    if seconds < 60 {
        format!("Opened {seconds}s ago")
    } else if seconds < 3600 {
        format!("Opened {}m ago", seconds / 60)
    } else if seconds < 86_400 {
        format!("Opened {}h ago", seconds / 3600)
    } else {
        format!("Opened {}d ago", seconds / 86_400)
    }
}
