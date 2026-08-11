//! Top window toolbar: navigation, repository context, command palette, open repository.

use gpui::{AnyElement, div, prelude::*, px};
use ui_kit::ThemeColors;

use crate::actions::{CommandPalette, NavigateBack, NavigateForward, OpenRepository, Refresh};
use crate::app_state::{GitronimoApp, RepositoryView, ShellState, WelcomeShellView};
use crate::views::components::{ActionTooltip, stacked_toolbar_button, toolbar_divider};
use crate::views::single_line_input::single_line_input_shell;
use git_domain::HeadStatus;

impl GitronimoApp {
    #[allow(clippy::too_many_lines)]
    #[allow(clippy::too_many_lines)]
    pub(crate) fn workspace_toolbar(
        &self,
        colors: &ThemeColors,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let (repo_name, branch_name, branch_info, view_label, changed) = match &self.state {
            ShellState::Welcome => {
                let title = match self.welcome_shell_view {
                    WelcomeShellView::Repositories => "Repositories",
                    WelcomeShellView::Services => "Services",
                    WelcomeShellView::Workflow => "Workflow",
                };
                (
                    title.into(),
                    String::new(),
                    String::new(),
                    String::new(),
                    0usize,
                )
            }
            ShellState::Loading(_) => (
                "Opening repository".into(),
                String::new(),
                String::new(),
                String::new(),
                0,
            ),
            ShellState::Repository(repository) => {
                let name = repository
                    .worktree_root
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("Repository")
                    .to_owned();
                let view = match self.repository_view {
                    RepositoryView::WorkingCopy => "Working Copy",
                    RepositoryView::History => "History",
                    RepositoryView::PullRequests => "Pull Requests",
                    RepositoryView::BranchesReview => "Branches Review",
                    RepositoryView::Services => "Services",
                    RepositoryView::Settings => "Settings",
                    RepositoryView::Stashes => "Stashes",
                    RepositoryView::Remotes => "Remotes",
                    _ => "Git workspace",
                };
                let groups = self.status_groups();
                let changed = groups.staged.len()
                    + groups.unstaged.len()
                    + groups.untracked.len()
                    + groups.conflicts.len();
                let (branch_name, branch_info) = self.working_copy.as_ref().map_or_else(
                    || (String::new(), String::new()),
                    |status| {
                        let branch = match &status.branch.head {
                            HeadStatus::Branch(name) => {
                                String::from_utf8_lossy(&name.0).into_owned()
                            }
                            HeadStatus::Detached => "Detached HEAD".into(),
                            HeadStatus::Unborn => "Unborn branch".into(),
                            HeadStatus::Unknown => "Unknown branch".into(),
                        };
                        let tracking = status.branch.upstream.as_ref().map_or_else(
                            || "No Tracking".to_owned(),
                            |upstream| String::from_utf8_lossy(&upstream.0).into_owned(),
                        );
                        let ahead = status.branch.ahead;
                        let behind = status.branch.behind;
                        let mut info = format!("{branch} \u{203A} {tracking}");
                        if ahead > 0 || behind > 0 {
                            info.push_str(" (");
                            let mut parts = Vec::new();
                            if ahead > 0 {
                                parts.push(format!("{ahead} \u{2191}"));
                            }
                            if behind > 0 {
                                parts.push(format!("{behind} \u{2193}"));
                            }
                            info.push_str(&parts.join(", "));
                            info.push(')');
                        }
                        (branch, info)
                    },
                );
                (name, branch_name, branch_info, view.to_owned(), changed)
            }
            ShellState::Error(_) => (
                "Repository needs attention".into(),
                String::new(),
                String::new(),
                String::new(),
                0,
            ),
        };
        let subtitle = if view_label.is_empty() {
            String::new()
        } else if changed > 0 && !branch_name.is_empty() {
            format!("{view_label} ({branch_name} - {changed} Changed Files)")
        } else if changed > 0 {
            format!("{view_label} ({changed} Changed Files)")
        } else if !branch_name.is_empty() {
            format!("{view_label} ({branch_name})")
        } else {
            view_label
        };
        let search_input = if matches!(self.state, ShellState::Repository(_)) {
            self.worktree_search_input.clone()
        } else {
            self.welcome_search_input.clone()
        };
        div()
            .h(px(52.0))
            .px_3()
            .flex()
            .items_center()
            .justify_between()
            .bg(colors.toolbar_background)
            .border_b_1()
            .border_color(colors.border)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(icon_toolbar_button(
                        "Quick Open",
                        "\u{2318}",
                        colors,
                        cx,
                        |app, _, cx| {
                            app.show_command_palette = false;
                            app.show_quick_open = !app.show_quick_open;
                            cx.notify();
                        },
                        false,
                    ))
                    .children(self.navigation_buttons(colors, cx)),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .text_base()
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .text_color(colors.text_primary)
                                    .child(repo_name),
                            )
                            .children((!branch_info.is_empty()).then(|| {
                                div()
                                    .text_xs()
                                    .text_color(colors.text_muted)
                                    .child("\u{203A}")
                                    .into_any_element()
                            }))
                            .children((!branch_info.is_empty()).then(|| {
                                div()
                                    .text_xs()
                                    .text_color(colors.text_secondary)
                                    .child(branch_info.clone())
                                    .into_any_element()
                            })),
                    )
                    .children((!subtitle.is_empty()).then(|| {
                        div()
                            .text_xs()
                            .text_color(colors.text_muted)
                            .child(subtitle)
                            .into_any_element()
                    })),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_0p5()
                    .children(self.repository_actions(colors, cx))
                    .children(self.working_copy_toolbar_actions(colors, cx))
                    .child(div().w(px(168.0)).child(single_line_input_shell(
                        search_input,
                        colors,
                        true,
                    )))
                    .child(toolbar_divider(colors))
                    .child(icon_toolbar_button(
                        "Palette",
                        "\u{2318}",
                        colors,
                        cx,
                        |_, window, cx| {
                            window.dispatch_action(Box::new(CommandPalette), cx);
                        },
                        false,
                    ))
                    .child(icon_toolbar_button(
                        "Open",
                        "+",
                        colors,
                        cx,
                        |_, window, cx| {
                            window.dispatch_action(Box::new(OpenRepository), cx);
                        },
                        false,
                    )),
            )
    }

    fn navigation_buttons(
        &self,
        colors: &ThemeColors,
        cx: &mut gpui::Context<Self>,
    ) -> Vec<AnyElement> {
        let has_back = !self.navigation_back.is_empty();
        let has_forward = !self.navigation_forward.is_empty();
        vec![
            labeled_toolbar_button(
                "Prev",
                "\u{2039}",
                colors,
                cx,
                |_, window, cx| {
                    window.dispatch_action(Box::new(NavigateBack), cx);
                },
                !has_back,
            ),
            labeled_toolbar_button(
                "Next",
                "\u{203A}",
                colors,
                cx,
                |_, window, cx| {
                    window.dispatch_action(Box::new(NavigateForward), cx);
                },
                !has_forward,
            ),
        ]
    }

    fn repository_actions(
        &self,
        colors: &ThemeColors,
        cx: &mut gpui::Context<Self>,
    ) -> Vec<AnyElement> {
        if !matches!(self.state, ShellState::Repository(_)) {
            return Vec::new();
        }
        vec![
            stacked_toolbar_button(
                "Fetch",
                "\u{2193}",
                colors,
                cx,
                |app, _, cx| {
                    app.fetch_default_remote(cx);
                },
                false,
            ),
            stacked_toolbar_button(
                "Pull",
                "\u{2913}",
                colors,
                cx,
                |app, _, cx| {
                    app.pull_current(cx);
                },
                false,
            ),
            stacked_toolbar_button(
                "Push",
                "\u{2B06}",
                colors,
                cx,
                |app, _, cx| {
                    app.push_current(cx);
                },
                false,
            ),
            stacked_toolbar_button(
                "Sync",
                "\u{27F3}",
                colors,
                cx,
                |app, _, cx| {
                    app.fetch_default_remote(cx);
                    app.pull_current(cx);
                    app.push_current(cx);
                },
                false,
            ),
            toolbar_divider(colors),
        ]
    }

    fn working_copy_toolbar_actions(
        &self,
        colors: &ThemeColors,
        cx: &mut gpui::Context<Self>,
    ) -> Vec<AnyElement> {
        if !matches!(self.state, ShellState::Repository(_))
            || self.repository_view != RepositoryView::WorkingCopy
        {
            return Vec::new();
        }
        vec![
            stacked_toolbar_button(
                "Apply",
                "\u{21A9}",
                colors,
                cx,
                |app, _, cx| {
                    app.apply_latest_stash(cx);
                },
                false,
            ),
            stacked_toolbar_button(
                "Save",
                "\u{21AA}",
                colors,
                cx,
                |app, _, cx| {
                    app.create_stash(false, cx);
                },
                false,
            ),
            stacked_toolbar_button(
                "Refresh",
                "\u{21BB}",
                colors,
                cx,
                |_, window, cx| {
                    window.dispatch_action(Box::new(Refresh), cx);
                },
                false,
            ),
            toolbar_divider(colors),
        ]
    }
}

#[allow(clippy::redundant_closure)]
fn labeled_toolbar_button(
    tooltip_label: &'static str,
    icon: &'static str,
    colors: &ThemeColors,
    cx: &mut gpui::Context<GitronimoApp>,
    on_click: impl Fn(&mut GitronimoApp, &mut gpui::Window, &mut gpui::Context<GitronimoApp>) + 'static,
    disabled: bool,
) -> AnyElement {
    let tooltip_colors = *colors;
    let icon_color = if disabled {
        colors.text_muted
    } else {
        colors.text_primary
    };
    div()
        .id(tooltip_label)
        .h(px(28.0))
        .px_1p5()
        .flex()
        .items_center()
        .gap_0p5()
        .rounded(px(4.0))
        .text_xs()
        .when(!disabled, |d| {
            #[allow(clippy::redundant_closure)]
            d.cursor_pointer()
        })
        .when(!disabled, |d| {
            d.tooltip(move |_, cx| {
                cx.new(|_| ActionTooltip {
                    label: tooltip_label,
                    colors: tooltip_colors,
                })
                .into()
            })
        })
        .when(!disabled, |d| {
            d.on_click(cx.listener(move |app, _, window, cx| {
                on_click(app, window, cx);
            }))
        })
        .text_color(icon_color)
        .child(icon)
        .child(tooltip_label)
        .into_any_element()
}

#[allow(clippy::redundant_closure)]
fn icon_toolbar_button(
    tooltip_label: &'static str,
    icon: &'static str,
    colors: &ThemeColors,
    cx: &mut gpui::Context<GitronimoApp>,
    on_click: impl Fn(&mut GitronimoApp, &mut gpui::Window, &mut gpui::Context<GitronimoApp>) + 'static,
    disabled: bool,
) -> AnyElement {
    let tooltip_colors = *colors;
    let icon_color = if disabled {
        colors.text_muted
    } else {
        colors.text_primary
    };
    div()
        .id(tooltip_label)
        .w(px(28.0))
        .h(px(28.0))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(4.0))
        .text_sm()
        .when(!disabled, |d| {
            #[allow(clippy::redundant_closure)]
            d.cursor_pointer()
        })
        .when(!disabled, |d| {
            d.tooltip(move |_, cx| {
                cx.new(|_| ActionTooltip {
                    label: tooltip_label,
                    colors: tooltip_colors,
                })
                .into()
            })
        })
        .when(!disabled, |d| {
            d.on_click(cx.listener(move |app, _, window, cx| {
                on_click(app, window, cx);
            }))
        })
        .text_color(icon_color)
        .child(icon)
        .into_any_element()
}
