//! Repositories view: grouped sidebar list plus a detail panel for the selection.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use gpui::{AnyElement, div, prelude::*, px};
use ui_kit::ThemeColors;

use crate::actions::OpenRepository;
use crate::app_state::{GitronimoApp, WelcomeRepoSnapshot, WelcomeShellView};
use crate::views::components::{
    file_action_button, primary_window_action_button, welcome_rail_tab,
};

impl GitronimoApp {
    pub(crate) fn welcome_vertical_rail(
        &self,
        colors: &ThemeColors,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        div()
            .w(px(56.0))
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
                .p_6()
                .bg(colors.window_background)
                .child(self.services_view(colors, cx))
                .into_any_element();
        }
        if self.welcome_shell_view == WelcomeShellView::Workflow {
            return welcome_workflow_placeholder(colors).into_any_element();
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
    welcome_repo_detail(path, index, app.welcome_snapshot.as_ref(), colors, cx)
}

fn welcome_empty_state(colors: &ThemeColors) -> AnyElement {
    div()
        .size_full()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap_2()
        .child(
            div()
                .text_lg()
                .font_weight(gpui::FontWeight::MEDIUM)
                .text_color(colors.text_primary)
                .child("Select a repository"),
        )
        .child(
            div()
                .text_sm()
                .text_color(colors.text_muted)
                .child("Choose a repository from the sidebar, or add one below."),
        )
        .child(
            div()
                .mt_2()
                .text_xs()
                .text_color(colors.text_muted)
                .child("You can also drop a folder anywhere on the window."),
        )
        .into_any_element()
}

#[allow(clippy::too_many_lines)]
fn welcome_repo_detail(
    path: &Path,
    index: usize,
    snapshot: Option<&WelcomeRepoSnapshot>,
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
                        .text_xl()
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(colors.text_primary)
                        .child(name),
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

fn welcome_workflow_placeholder(colors: &ThemeColors) -> AnyElement {
    div()
        .size_full()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap_2()
        .bg(colors.window_background)
        .child(
            div()
                .text_lg()
                .font_weight(gpui::FontWeight::MEDIUM)
                .text_color(colors.text_primary)
                .child("Workflow view coming soon"),
        )
        .child(
            div()
                .text_sm()
                .text_color(colors.text_muted)
                .child("Open a repository to review changes and commit from Working Copy."),
        )
        .into_any_element()
}

fn detail_section(title: &'static str, colors: &ThemeColors) -> AnyElement {
    div()
        .pt_4()
        .pb_1()
        .text_xs()
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .text_color(colors.text_muted)
        .child(title.to_uppercase())
        .into_any_element()
}

fn detail_row(label: &str, value: &str, colors: &ThemeColors) -> AnyElement {
    div()
        .py_1p5()
        .border_b_1()
        .border_color(colors.separator)
        .flex()
        .gap_4()
        .child(
            div()
                .w(px(140.0))
                .flex_shrink_0()
                .text_sm()
                .text_color(colors.text_muted)
                .child(label.to_owned()),
        )
        .child(
            div()
                .flex_1()
                .text_sm()
                .text_color(colors.text_primary)
                .child(value.to_owned()),
        )
        .into_any_element()
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

fn format_last_opened(index: usize, modified: Option<SystemTime>) -> String {
    if index == 0 {
        return "Most recently opened".into();
    }
    let Some(modified) = modified else {
        return "Unknown".into();
    };
    let Ok(age) = SystemTime::now().duration_since(modified) else {
        return "Unknown".into();
    };
    let seconds = age.as_secs();
    if seconds < 60 {
        format!("Updated {seconds}s ago")
    } else if seconds < 3600 {
        format!("Updated {}m ago", seconds / 60)
    } else if seconds < 86_400 {
        format!("Updated {}h ago", seconds / 3600)
    } else {
        format!("Updated {}d ago", seconds / 86_400)
    }
}
