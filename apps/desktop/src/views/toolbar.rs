//! Top window toolbar: navigation, repository context, command palette, open repository.

use gpui::{AnyElement, div, prelude::*, px};
use ui_kit::ThemeColors;

use crate::actions::{NavigateBack, NavigateForward, OpenRepository, Refresh};
use crate::app_state::{GitronimoApp, RepositoryView, ShellState, WelcomeShellView};
use crate::views::components::{
    ActionTooltip, format_divergence_arrows, stacked_toolbar_button, toolbar_divider,
};
use crate::views::icons::{IconKind, icon};
use crate::views::single_line_input::toolbar_search_shell;
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
                    RepositoryView::Settings => "Settings",
                    RepositoryView::Workflow => "Workflow",
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
                        if let Some(divergence) = format_divergence_arrows(ahead, behind) {
                            info.push_str(" (");
                            info.push_str(&divergence);
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
            .h(px(56.0))
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
                    .gap_3()
                    .min_w(px(0.0))
                    .overflow_hidden()
                    .child(self.shell_tabs(colors, cx))
                    .child(toolbar_divider(colors))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1()
                            .flex_shrink_0()
                            .child(icon_toolbar_button(
                                "Quick Open",
                                IconKind::Grid,
                                colors,
                                cx,
                                |app, _, cx| {
                                    app.show_command_palette = false;
                                    app.welcome_plus_menu_open = false;
                                    app.show_quick_open = !app.show_quick_open;
                                    cx.notify();
                                },
                                false,
                            ))
                            .children(
                                matches!(self.state, ShellState::Repository(_))
                                    .then(|| self.navigation_buttons(colors, cx))
                                    .into_iter()
                                    .flatten(),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .items_start()
                            .min_w(px(0.0))
                            .overflow_hidden()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .overflow_hidden()
                                    .child(
                                        div()
                                            .text_base()
                                            .font_weight(gpui::FontWeight::SEMIBOLD)
                                            .text_color(colors.text_primary)
                                            .whitespace_nowrap()
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
                                            .whitespace_nowrap()
                                            .overflow_hidden()
                                            .child(branch_info.clone())
                                            .into_any_element()
                                    })),
                            )
                            .children((!subtitle.is_empty()).then(|| {
                                div()
                                    .text_xs()
                                    .text_color(colors.text_muted)
                                    .whitespace_nowrap()
                                    .overflow_hidden()
                                    .child(subtitle)
                                    .into_any_element()
                            })),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_0p5()
                    .flex_shrink_0()
                    .children(self.repository_actions(colors, cx))
                    .children(self.working_copy_toolbar_actions(colors, cx))
                    .child(div().w(px(220.0)).child(toolbar_search_shell(
                        search_input,
                        colors,
                        matches!(self.state, ShellState::Welcome),
                    )))
                    .child(toolbar_divider(colors))
                    .child(icon_toolbar_button(
                        "Palette",
                        IconKind::Palette,
                        colors,
                        cx,
                        |app, _, cx| {
                            app.open_command_palette(cx);
                        },
                        false,
                    ))
                    .child(icon_toolbar_button(
                        "Open",
                        IconKind::Plus,
                        colors,
                        cx,
                        |_, window, cx| {
                            window.dispatch_action(Box::new(OpenRepository), cx);
                        },
                        false,
                    )),
            )
    }

    fn shell_tabs(&self, colors: &ThemeColors, cx: &mut gpui::Context<Self>) -> AnyElement {
        let on_welcome = matches!(self.state, ShellState::Welcome);
        let workflow_active = (on_welcome && self.welcome_shell_view == WelcomeShellView::Workflow)
            || (!on_welcome && self.repository_view == RepositoryView::Workflow);
        div()
            .flex()
            .items_center()
            .gap_1()
            .flex_shrink_0()
            .child(
                div()
                    .id("shell-tab-spacer")
                    .w(px(72.0))
                    .h(px(48.0))
                    .flex_shrink_0(),
            )
            .child(shell_tab_button(
                "Bookmarks",
                IconKind::Bookmark,
                on_welcome && self.welcome_shell_view == WelcomeShellView::Repositories,
                colors,
                cx,
                |app, cx| app.set_welcome_shell_view(WelcomeShellView::Repositories, cx),
            ))
            .child(shell_tab_button(
                "Workflow",
                IconKind::Workflow,
                workflow_active,
                colors,
                cx,
                GitronimoApp::open_workflow,
            ))
            .into_any_element()
    }

    fn navigation_buttons(
        &self,
        colors: &ThemeColors,
        cx: &mut gpui::Context<Self>,
    ) -> Vec<AnyElement> {
        let has_back = !self.navigation_back.is_empty();
        let has_forward = !self.navigation_forward.is_empty();
        vec![
            icon_toolbar_button(
                "Prev",
                IconKind::ChevronLeft,
                colors,
                cx,
                |_, window, cx| {
                    window.dispatch_action(Box::new(NavigateBack), cx);
                },
                !has_back,
            ),
            icon_toolbar_button(
                "Next",
                IconKind::ChevronRight,
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
                IconKind::Fetch,
                colors,
                cx,
                |app, _, cx| {
                    app.fetch_default_remote(cx);
                },
                false,
            ),
            stacked_toolbar_button(
                "Pull",
                IconKind::Pull,
                colors,
                cx,
                |app, _, cx| {
                    app.pull_current(cx);
                },
                false,
            ),
            stacked_toolbar_button(
                "Push",
                IconKind::Push,
                colors,
                cx,
                |app, _, cx| {
                    app.push_current(cx);
                },
                false,
            ),
            stacked_toolbar_button(
                "Sync",
                IconKind::Sync,
                colors,
                cx,
                |app, _, cx| {
                    app.sync_current(cx);
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
                IconKind::StashApply,
                colors,
                cx,
                |app, _, cx| {
                    app.open_apply_latest_stash_dialog(cx);
                },
                false,
            ),
            stacked_toolbar_button(
                "Save",
                IconKind::StashSave,
                colors,
                cx,
                |app, _, cx| {
                    app.open_stash_save_dialog(false, Vec::new(), cx);
                },
                false,
            ),
            stacked_toolbar_button(
                "Refresh",
                IconKind::Sync,
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

fn shell_tab_button(
    label: &'static str,
    kind: IconKind,
    active: bool,
    colors: &ThemeColors,
    cx: &mut gpui::Context<GitronimoApp>,
    on_click: impl Fn(&mut GitronimoApp, &mut gpui::Context<GitronimoApp>) + 'static,
) -> AnyElement {
    let icon_color = if active {
        colors.text_primary
    } else {
        colors.text_muted
    };
    div()
        .id(gpui::ElementId::Name(
            format!("shell-tab-button:{label}").into(),
        ))
        .w(px(72.0))
        .h(px(48.0))
        .px_2()
        .py_1()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap_0p5()
        .rounded(px(6.0))
        .cursor_pointer()
        .bg(if active {
            colors.raised_background
        } else {
            colors.toolbar_background
        })
        .hover(|style| style.bg(colors.selection))
        .on_click(cx.listener(move |app, _, _, cx| on_click(app, cx)))
        .child(icon(kind, 18.0, icon_color))
        .child(
            div()
                .text_xs()
                .whitespace_nowrap()
                .text_center()
                .text_color(if active {
                    colors.text_primary
                } else {
                    colors.text_muted
                })
                .font_weight(if active {
                    gpui::FontWeight::MEDIUM
                } else {
                    gpui::FontWeight::NORMAL
                })
                .child(label),
        )
        .into_any_element()
}

#[allow(clippy::redundant_closure)]
fn icon_toolbar_button(
    tooltip_label: &'static str,
    kind: IconKind,
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
        .id(gpui::ElementId::Name(
            format!("icon-toolbar-button:{tooltip_label}").into(),
        ))
        .w(px(28.0))
        .h(px(28.0))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(4.0))
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
        .child(icon(kind, 16.0, icon_color))
        .into_any_element()
}
