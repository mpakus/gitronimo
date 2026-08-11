//! Left sidebar: source-list navigation, status badges, and ref trees.

use std::path::PathBuf;

use gpui::{AnyElement, ClickEvent, MouseButton, Render, Window, div, prelude::*, px};
use ui_kit::ThemeColors;

use git_domain::HeadStatus;
use git_domain::NamedRef;

use crate::app_state::{
    ChoicePromptKind, GitronimoApp, RefContext, RefKind, WelcomeRepoSnapshot, WelcomeShellView,
};
use crate::views::components::{
    NAV_ROW_HEIGHT, count_badge, head_badge, remote_progress_footer, sidebar_section_label,
};
use crate::views::icons::{IconKind, icon};

/// Distinct drag type for bookmark repository rows (does not cross-fire with pane dividers).
#[derive(Clone)]
struct BookmarkRepoDrag {
    path: PathBuf,
}

impl Render for BookmarkRepoDrag {
    fn render(&mut self, _: &mut Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
        div().w(px(0.0)).h(px(0.0))
    }
}

impl GitronimoApp {
    #[allow(clippy::too_many_lines)]
    pub(crate) fn sidebar_view(
        &self,
        width: f32,
        colors: &ThemeColors,
        cx: &mut gpui::Context<Self>,
    ) -> AnyElement {
        if matches!(self.state, crate::app_state::ShellState::Welcome) {
            return welcome_sidebar_view(self, width, colors, cx);
        }
        let current_branch =
            self.working_copy
                .as_ref()
                .and_then(|status| match &status.branch.head {
                    HeadStatus::Branch(name) => String::from_utf8(name.0.clone()).ok(),
                    _ => None,
                });
        let groups = self.status_groups();
        let groups_total = groups.staged.len()
            + groups.unstaged.len()
            + groups.untracked.len()
            + groups.conflicts.len();
        div()
            .w(px(width))
            .h_full()
            .flex_shrink_0()
            .overflow_hidden()
            .flex()
            .flex_col()
            .bg(colors.sidebar_background)
            .child(sidebar_section_label("WORKSPACE", colors))
            .child(nav_row_with_badge(
                "Working Copy",
                "\u{25A4}",
                "sidebar-working-copy",
                self.repository_view == crate::app_state::RepositoryView::WorkingCopy,
                if groups_total > 0 {
                    Some(groups_total.to_string())
                } else {
                    None
                },
                colors,
                cx,
                |app, _, cx| {
                    app.navigate_to(crate::app_state::RepositoryView::WorkingCopy, cx);
                },
            ))
            .child(nav_row(
                "History",
                "\u{25F7}",
                "sidebar-history",
                self.repository_view == crate::app_state::RepositoryView::History,
                colors,
                cx,
                |app, _, cx| {
                    app.navigate_to(crate::app_state::RepositoryView::History, cx);
                },
            ))
            .child(nav_row(
                "Stashes",
                "\u{21A9}",
                "sidebar-stashes",
                self.repository_view == crate::app_state::RepositoryView::Stashes,
                colors,
                cx,
                |app, _, cx| {
                    if let crate::app_state::ShellState::Repository(repository) = &app.state {
                        app.show_stashes(repository.clone(), cx);
                    }
                },
            ))
            .child(nav_row(
                "Pull Requests",
                "\u{2194}",
                "sidebar-pull-requests",
                self.repository_view == crate::app_state::RepositoryView::PullRequests,
                colors,
                cx,
                |app, _, cx| {
                    if let Some(repo) = app.pull_request_repository.clone() {
                        app.show_pull_requests(repo, cx);
                    } else {
                        app.navigate_to(crate::app_state::RepositoryView::PullRequests, cx);
                    }
                },
            ))
            .child(nav_row(
                "Branches Review",
                "\u{2696}",
                "sidebar-branches-review",
                self.repository_view == crate::app_state::RepositoryView::BranchesReview,
                colors,
                cx,
                |app, _, cx| {
                    app.navigate_to(crate::app_state::RepositoryView::BranchesReview, cx);
                },
            ))
            .child(nav_row(
                "Reflog",
                "\u{21BA}",
                "sidebar-reflog",
                self.repository_view == crate::app_state::RepositoryView::Reflog,
                colors,
                cx,
                |app, _, cx| {
                    if let crate::app_state::ShellState::Repository(repository) = &app.state {
                        app.show_reflog(repository.clone(), cx);
                    }
                },
            ))
            .child(nav_row(
                "Settings",
                "\u{2699}",
                "sidebar-settings",
                self.repository_view == crate::app_state::RepositoryView::Settings,
                colors,
                cx,
                |app, _, cx| {
                    app.navigate_to(crate::app_state::RepositoryView::Settings, cx);
                },
            ))
            .child(
                div()
                    .flex_1()
                    .overflow_hidden()
                    .flex()
                    .flex_col()
                    .child(sidebar_section_label("BRANCHES", colors))
                    .children(self.ref_rows(
                        "local",
                        &self.refs.local_branches,
                        RefKind::LocalBranch,
                        current_branch.as_deref(),
                        colors,
                        cx,
                    ))
                    .child(sidebar_section_label("TAGS", colors))
                    .children(self.ref_rows("tag", &self.refs.tags, RefKind::Tag, None, colors, cx))
                    .child(sidebar_section_label("REMOTES", colors))
                    .child(nav_row(
                        "Remotes",
                        "\u{2601}",
                        "sidebar-remotes",
                        self.repository_view == crate::app_state::RepositoryView::Remotes,
                        colors,
                        cx,
                        |app, _, cx| {
                            app.show_remotes(cx);
                        },
                    ))
                    .children(self.ref_rows(
                        "remote",
                        &self.refs.remote_branches,
                        RefKind::RemoteBranch,
                        None,
                        colors,
                        cx,
                    ))
                    .children(self.refs.remotes.iter().enumerate().filter_map(
                        |(index, remote)| {
                            String::from_utf8(remote.name.0.clone()).ok().map(|name| {
                                let context = RefContext::Remote(name.clone());
                                div()
                                    .id(("remote-ref", index))
                                    .h(px(22.0))
                                    .px_3()
                                    .pl_6()
                                    .flex()
                                    .items_center()
                                    .text_xs()
                                    .text_color(colors.text_secondary)
                                    .cursor_pointer()
                                    .on_click(cx.listener(move |app, _, _, cx| {
                                        app.select_ref_context(context.clone(), cx);
                                    }))
                                    .child(name)
                                    .into_any_element()
                            })
                        },
                    )),
            )
            .children(self.remote_activity_footer(colors, cx))
            .into_any_element()
    }

    fn remote_activity_footer(
        &self,
        colors: &ThemeColors,
        cx: &mut gpui::Context<Self>,
    ) -> Option<AnyElement> {
        if let Some(operation) = self.network_operation.as_ref() {
            let label = operation.lock().ok()?.label.clone();
            return Some(remote_progress_footer(
                &label,
                self.network_progress,
                colors,
                cx,
            ));
        }
        self.last_network_result.as_ref().map(|result| {
            let color = if result.contains("complete") {
                colors.success
            } else if result.contains("failed") || result.contains("cancelled") {
                colors.text_muted
            } else {
                colors.text_secondary
            };
            div()
                .mt_auto()
                .mx_3()
                .mb_3()
                .px_2()
                .py_1p5()
                .rounded(px(4.0))
                .bg(colors.raised_background)
                .text_xs()
                .text_color(color)
                .child(result.clone())
                .into_any_element()
        })
    }

    #[allow(dead_code, clippy::unused_self)]
    fn repositories_section(
        &self,
        colors: &ThemeColors,
        _cx: &mut gpui::Context<Self>,
    ) -> AnyElement {
        div()
            .px_3()
            .pt_3()
            .pb_2()
            .text_xs()
            .text_color(colors.text_muted)
            .child("Repositories")
            .into_any_element()
    }

    #[allow(dead_code)]
    fn services_section(&self, colors: &ThemeColors, _cx: &mut gpui::Context<Self>) -> AnyElement {
        let mut children = Vec::new();
        if let Some(account) = &self.service_account {
            children.push(
                div()
                    .px_3()
                    .py_2()
                    .h(px(24.0))
                    .flex()
                    .items_center()
                    .gap_2()
                    .text_sm()
                    .cursor_pointer()
                    .child(account.login.clone())
                    .into_any_element(),
            );
        }
        if children.is_empty() {
            children.push(
                div()
                    .px_3()
                    .py_2()
                    .h(px(24.0))
                    .flex()
                    .items_center()
                    .gap_2()
                    .text_sm()
                    .text_color(colors.text_muted)
                    .child("Add Service")
                    .into_any_element(),
            );
        }
        div()
            .px_3()
            .pt_4()
            .pb_2()
            .text_xs()
            .text_color(colors.text_muted)
            .child("Services")
            .into_any_element()
    }

    fn ref_rows(
        &self,
        category: &str,
        refs: &[NamedRef],
        kind: RefKind,
        current_branch: Option<&str>,
        colors: &ThemeColors,
        cx: &mut gpui::Context<Self>,
    ) -> Vec<AnyElement> {
        let mut groups = std::collections::BTreeSet::new();
        let mut rows = Vec::new();
        let id_prefix = match category {
            "local" => "local-ref",
            "remote" => "remote-branch-ref",
            _ => "tag-ref",
        };
        let group_id_prefix = match category {
            "local" => "local-ref-group",
            "remote" => "remote-ref-group",
            _ => "tag-ref-group",
        };
        for reference in refs {
            let Ok(name) = String::from_utf8(reference.name.0.clone()) else {
                continue;
            };
            let parts: Vec<_> = name.split('/').collect();
            let mut visible = true;
            for depth in 1..parts.len() {
                let group = parts[..depth].join("/");
                let key = format!("{category}:{group}");
                let expanded = self.expanded_ref_groups.contains(&key);
                if groups.insert(key.clone()) {
                    let label = format!(
                        "{}{} {}",
                        "  ".repeat(depth),
                        if expanded { "\u{2304}" } else { "\u{203A}" },
                        group.rsplit('/').next().unwrap_or_default()
                    );
                    rows.push(
                        div()
                            .id((group_id_prefix, rows.len()))
                            .h(px(22.0))
                            .px_3()
                            .flex()
                            .items_center()
                            .text_sm()
                            .text_color(colors.text_secondary)
                            .cursor_pointer()
                            .on_click(cx.listener(move |app, _, _, cx| {
                                app.toggle_ref_group(key.clone(), cx);
                            }))
                            .child(label)
                            .into_any_element(),
                    );
                }
                visible &= expanded;
                if !visible {
                    break;
                }
            }
            if visible {
                let context = kind.context(name.clone());
                let indent = u16::try_from(parts.len().saturating_mul(12)).unwrap_or(u16::MAX);
                let is_head = matches!(kind, RefKind::LocalBranch)
                    && current_branch.is_some_and(|branch| branch == name);
                rows.push(
                    div()
                        .id((id_prefix, rows.len()))
                        .h(px(22.0))
                        .px_3()
                        .pl(px(f32::from(indent)))
                        .flex()
                        .items_center()
                        .gap_2()
                        .text_xs()
                        .text_color(colors.text_secondary)
                        .cursor_pointer()
                        .hover(|style| style.bg(colors.selection))
                        .on_click(cx.listener(move |app, _, _, cx| {
                            app.select_ref_context(context.clone(), cx);
                        }))
                        .child(parts.last().copied().unwrap_or_default().to_owned())
                        .children(is_head.then(|| head_badge(colors)))
                        .into_any_element(),
                );
            }
        }
        rows
    }
}

fn nav_row(
    label: &'static str,
    icon: &'static str,
    id: &'static str,
    active: bool,
    colors: &ThemeColors,
    cx: &mut gpui::Context<GitronimoApp>,
    on_click: impl Fn(&mut GitronimoApp, &ClickEvent, &mut gpui::Context<GitronimoApp>) + 'static,
) -> AnyElement {
    div()
        .id(id)
        .h(px(NAV_ROW_HEIGHT))
        .px_3()
        .flex()
        .items_center()
        .gap_2()
        .text_sm()
        .bg(if active {
            colors.accent
        } else {
            colors.sidebar_background
        })
        .text_color(if active {
            colors.panel_background
        } else {
            colors.text_primary
        })
        .cursor_pointer()
        .when(!active, |row| row.hover(|style| style.bg(colors.selection)))
        .on_click(cx.listener(move |app, event, _, cx| on_click(app, event, cx)))
        .child(
            div()
                .w(px(14.0))
                .text_xs()
                .text_color(if active {
                    colors.panel_background
                } else {
                    colors.accent
                })
                .child(icon),
        )
        .child(label)
        .into_any_element()
}

#[allow(clippy::too_many_arguments)]
fn nav_row_with_badge(
    label: &'static str,
    icon: &'static str,
    id: &'static str,
    active: bool,
    badge: Option<String>,
    colors: &ThemeColors,
    cx: &mut gpui::Context<GitronimoApp>,
    on_click: impl Fn(&mut GitronimoApp, &ClickEvent, &mut gpui::Context<GitronimoApp>) + 'static,
) -> AnyElement {
    div()
        .id(id)
        .h(px(NAV_ROW_HEIGHT))
        .px_3()
        .flex()
        .items_center()
        .gap_2()
        .text_sm()
        .bg(if active {
            colors.accent
        } else {
            colors.sidebar_background
        })
        .text_color(if active {
            colors.panel_background
        } else {
            colors.text_primary
        })
        .cursor_pointer()
        .when(!active, |row| row.hover(|style| style.bg(colors.selection)))
        .on_click(cx.listener(move |app, event, _, cx| on_click(app, event, cx)))
        .child(
            div()
                .w(px(14.0))
                .text_xs()
                .text_color(if active {
                    colors.panel_background
                } else {
                    colors.accent
                })
                .child(icon),
        )
        .child(label)
        .children(badge.map(|text| count_badge(text, active, colors)))
        .into_any_element()
}

#[allow(clippy::too_many_lines)]
fn welcome_sidebar_view(
    app: &GitronimoApp,
    width: f32,
    colors: &ThemeColors,
    cx: &mut gpui::Context<GitronimoApp>,
) -> AnyElement {
    if app.welcome_shell_view == WelcomeShellView::Services {
        return welcome_services_sidebar(app, width, colors);
    }
    if app.welcome_shell_view == WelcomeShellView::Workflow {
        return welcome_workflow_sidebar(width, colors);
    }

    let search = app.welcome_repo_search.to_lowercase();
    let searching = !search.is_empty();
    let mut rows = Vec::new();

    let valid_folder_ids: std::collections::HashSet<&str> = app
        .bookmark_folders
        .iter()
        .map(|folder| folder.id.as_str())
        .collect();

    let repo_matches = |path: &std::path::Path| -> bool {
        if !searching {
            return true;
        }
        path.file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.to_lowercase().contains(&search))
    };

    let folder_id_for = |path: &std::path::Path| -> Option<&str> {
        app.repository_folders
            .get(path)
            .map(String::as_str)
            .filter(|id| valid_folder_ids.contains(id))
    };

    for (folder_index, folder) in app.bookmark_folders.iter().enumerate() {
        let children: Vec<(usize, PathBuf)> = app
            .recents
            .iter()
            .cloned()
            .enumerate()
            .filter(|(_, path)| folder_id_for(path) == Some(folder.id.as_str()))
            .filter(|(_, path)| repo_matches(path))
            .collect();

        if searching && children.is_empty() {
            continue;
        }

        let folder_id = folder.id.clone();
        let expanded = folder.expanded || searching;
        let chevron = if expanded {
            IconKind::ChevronDown
        } else {
            IconKind::ChevronRight
        };
        let toggle_id = folder_id.clone();
        let menu_id = folder_id.clone();
        let drop_id = folder_id.clone();

        rows.push(
            div()
                .id(("welcome-folder", folder_index))
                .h(px(NAV_ROW_HEIGHT))
                .px_3()
                .flex()
                .items_center()
                .gap_2()
                .text_sm()
                .text_color(colors.text_primary)
                .cursor_pointer()
                .hover(|style| style.bg(colors.selection))
                .on_click(cx.listener(move |app, _, _, cx| {
                    app.toggle_bookmark_folder(&toggle_id, cx);
                }))
                .on_mouse_down(
                    MouseButton::Right,
                    cx.listener(move |app, _, _, cx| {
                        app.begin_choice_prompt(
                            ChoicePromptKind::BookmarkFolderActions {
                                id: menu_id.clone(),
                            },
                            cx,
                        );
                    }),
                )
                .on_drop(cx.listener(move |app, drag: &BookmarkRepoDrag, _, cx| {
                    app.move_repository_to_folder(drag.path.clone(), Some(drop_id.clone()), cx);
                }))
                .child(icon(chevron, 12.0, colors.text_muted))
                .child(icon(IconKind::Folder, 14.0, colors.text_muted))
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.0))
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .child(folder.name.clone()),
                )
                .into_any_element(),
        );

        if expanded {
            for (index, path) in children {
                rows.push(welcome_repo_row(app, index, &path, true, colors, cx));
            }
        }
    }

    let root_repos: Vec<(usize, PathBuf)> = app
        .recents
        .iter()
        .cloned()
        .enumerate()
        .filter(|(_, path)| folder_id_for(path).is_none())
        .filter(|(_, path)| repo_matches(path))
        .collect();

    for (index, path) in root_repos {
        rows.push(welcome_repo_row(app, index, &path, false, colors, cx));
    }

    div()
        .w(px(width))
        .h_full()
        .flex_shrink_0()
        .overflow_hidden()
        .flex()
        .flex_col()
        .bg(colors.sidebar_background)
        .child(
            div().px_3().pt_3().pb_2().flex().items_center().child(
                div()
                    .text_xs()
                    .text_color(colors.text_muted)
                    .child("Repositories"),
            ),
        )
        .child(
            div()
                .id("welcome-bookmark-list")
                .flex_1()
                .min_h(px(0.0))
                .overflow_hidden()
                .flex()
                .flex_col()
                .on_drop(cx.listener(|app, drag: &BookmarkRepoDrag, _, cx| {
                    app.move_repository_to_folder(drag.path.clone(), None, cx);
                }))
                .children(rows),
        )
        .child(
            div()
                .h(px(40.0))
                .px_3()
                .py_2()
                .flex()
                .items_center()
                .border_t_1()
                .border_color(colors.border)
                .bg(colors.sidebar_background)
                .on_drop(cx.listener(|app, drag: &BookmarkRepoDrag, _, cx| {
                    app.move_repository_to_folder(drag.path.clone(), None, cx);
                }))
                .child(
                    div()
                        .id("welcome-sidebar-add")
                        .w(px(28.0))
                        .h(px(28.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(6.0))
                        .cursor_pointer()
                        .hover(|style| style.bg(colors.raised_background))
                        .on_click(cx.listener(|app, _, _, cx| {
                            app.begin_choice_prompt(ChoicePromptKind::WelcomeSidebarPlus, cx);
                        }))
                        .child(icon(IconKind::Plus, 16.0, colors.text_secondary)),
                ),
        )
        .into_any_element()
}

fn welcome_workflow_sidebar(width: f32, colors: &ThemeColors) -> AnyElement {
    div()
        .w(px(width))
        .h_full()
        .flex_shrink_0()
        .overflow_hidden()
        .flex()
        .flex_col()
        .bg(colors.sidebar_background)
        .child(
            div()
                .px_3()
                .pt_3()
                .pb_2()
                .text_xs()
                .text_color(colors.text_muted)
                .child("Workflow"),
        )
        .child(
            div()
                .px_3()
                .py_2()
                .text_sm()
                .text_color(colors.text_muted)
                .child("No workflow items yet."),
        )
        .into_any_element()
}

fn welcome_services_sidebar(app: &GitronimoApp, width: f32, colors: &ThemeColors) -> AnyElement {
    let account = app.service_account.as_ref().map_or_else(
        || "No account connected".to_owned(),
        |account| account.login.clone(),
    );
    div()
        .w(px(width))
        .h_full()
        .flex_shrink_0()
        .overflow_hidden()
        .flex()
        .flex_col()
        .bg(colors.sidebar_background)
        .child(
            div()
                .px_3()
                .pt_3()
                .pb_2()
                .text_xs()
                .text_color(colors.text_muted)
                .child("Services"),
        )
        .child(
            div()
                .px_3()
                .py_2()
                .text_sm()
                .text_color(colors.text_secondary)
                .child(account),
        )
        .into_any_element()
}

fn welcome_repo_row(
    app: &GitronimoApp,
    index: usize,
    path: &std::path::Path,
    nested: bool,
    colors: &ThemeColors,
    cx: &mut gpui::Context<GitronimoApp>,
) -> AnyElement {
    let path = path.to_path_buf();
    let drag_path = path.clone();
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("Repository")
        .to_owned();
    let selected = app.selected_recent == Some(index);
    let snapshot = app.welcome_list_snapshots.get(&path);
    let branch_hint = snapshot.and_then(|data| data.branch.as_deref());
    let upstream_badge = snapshot.and_then(welcome_upstream_badge);
    let trailing = upstream_badge.or_else(|| branch_hint.map(str::to_owned));
    let text_primary = if selected {
        colors.panel_background
    } else {
        colors.text_primary
    };
    let text_secondary = if selected {
        colors.panel_background
    } else {
        colors.text_muted
    };
    let left_pad = if nested { px(28.0) } else { px(12.0) };
    div()
        .id(("welcome-repository", index))
        .h(px(NAV_ROW_HEIGHT))
        .pl(left_pad)
        .pr_3()
        .flex()
        .items_center()
        .gap_2()
        .text_sm()
        .bg(if selected {
            colors.accent
        } else {
            colors.sidebar_background
        })
        .text_color(text_primary)
        .cursor_pointer()
        .when(!selected, |row| {
            row.hover(|style| style.bg(colors.selection))
        })
        .on_click(cx.listener(move |app, event: &ClickEvent, window, cx| {
            app.select_recent(index, cx);
            if event.click_count() >= 2 {
                app.open_recent(path.clone(), window, cx);
            }
        }))
        .on_drag(
            BookmarkRepoDrag { path: drag_path },
            |drag, _offset, _, cx| cx.new(|_| drag.clone()),
        )
        .child(icon(
            IconKind::Repo,
            14.0,
            if selected {
                colors.panel_background
            } else {
                colors.text_muted
            },
        ))
        .child(
            div()
                .flex_1()
                .min_w(px(0.0))
                .overflow_hidden()
                .whitespace_nowrap()
                .child(name),
        )
        .children(trailing.map(|text| {
            div()
                .flex_shrink_0()
                .max_w(px(100.0))
                .overflow_hidden()
                .whitespace_nowrap()
                .text_xs()
                .text_color(text_secondary)
                .child(text)
                .into_any_element()
        }))
        .into_any_element()
}

fn welcome_upstream_badge(snapshot: &WelcomeRepoSnapshot) -> Option<String> {
    if snapshot.ahead == 0 && snapshot.behind == 0 {
        return None;
    }
    let mut parts = Vec::new();
    if snapshot.ahead > 0 {
        parts.push(format!("{} \u{2191}", snapshot.ahead));
    }
    if snapshot.behind > 0 {
        parts.push(format!("{} \u{2193}", snapshot.behind));
    }
    Some(parts.join(" "))
}
