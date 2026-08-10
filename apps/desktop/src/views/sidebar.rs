//! Left sidebar: source-list navigation, status badges, and ref trees.

use gpui::{AnyElement, ClickEvent, div, prelude::*, px};
use ui_kit::ThemeColors;

use git_domain::NamedRef;

use crate::actions::OpenRepository;
use crate::app_state::{GitronimoApp, RefContext, RefKind};

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
        let groups = self.status_groups();
        let groups_total = groups.staged.len()
            + groups.unstaged.len()
            + groups.untracked.len()
            + groups.conflicts.len();
        div()
            .w(px(width))
            .h_full()
            .flex()
            .flex_col()
            .bg(colors.sidebar_background)
            .child(self.repositories_section(colors, cx))
            .child(self.services_section(colors, cx))
            .child(
                div()
                    .px_3()
                    .pt_3()
                    .pb_2()
                    .text_xs()
                    .text_color(colors.text_muted)
                    .child("Workspace"),
            )
            .child(nav_row_with_badge(
                "Working Copy",
                "📂",
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
                "🕐",
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
                "📦",
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
                "🔀",
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
                "Reflog",
                "📋",
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
                "⚙",
                "sidebar-settings",
                self.repository_view == crate::app_state::RepositoryView::Services,
                colors,
                cx,
                |app, _, cx| {
                    app.show_services(cx);
                },
            ))
            .child(
                div()
                    .px_3()
                    .pt_4()
                    .pb_2()
                    .text_xs()
                    .text_color(colors.text_muted)
                    .child("Branches"),
            )
            .children(self.ref_rows(
                "local",
                &self.refs.local_branches,
                RefKind::LocalBranch,
                colors,
                cx,
            ))
            .child(
                div()
                    .px_3()
                    .pt_2()
                    .pb_2()
                    .text_xs()
                    .text_color(colors.text_muted)
                    .child("Remote"),
            )
            .children(self.ref_rows(
                "remote",
                &self.refs.remote_branches,
                RefKind::RemoteBranch,
                colors,
                cx,
            ))
            .child(
                div()
                    .px_3()
                    .pt_4()
                    .pb_2()
                    .text_xs()
                    .text_color(colors.text_muted)
                    .child("Tags"),
            )
            .children(self.ref_rows("tag", &self.refs.tags, RefKind::Tag, colors, cx))
            .child(nav_row(
                "Remotes",
                "🌐",
                "sidebar-remotes",
                self.repository_view == crate::app_state::RepositoryView::Remotes,
                colors,
                cx,
                |app, _, cx| {
                    app.show_remotes(cx);
                },
            ))
            .children(
                self.refs
                    .remotes
                    .iter()
                    .enumerate()
                    .filter_map(|(index, remote)| {
                        String::from_utf8(remote.name.0.clone()).ok().map(|name| {
                            let context = RefContext::Remote(name.clone());
                            div()
                                .id(("remote-ref", index))
                                .px_3()
                                .pl_6()
                                .h(px(22.0))
                                .flex()
                                .items_center()
                                .text_sm()
                                .cursor_pointer()
                                .on_click(cx.listener(move |app, _, _, cx| {
                                    app.select_ref_context(context.clone(), cx);
                                }))
                                .child(name)
                                .into_any_element()
                        })
                    }),
            )
            .children(self.remote_activity_footer(colors, cx))
            .into_any_element()
    }

    fn remote_activity_footer(
        &self,
        colors: &ThemeColors,
        cx: &mut gpui::Context<Self>,
    ) -> Option<AnyElement> {
        let operation = self.network_operation.as_ref()?;
        let label = operation.lock().ok()?.label.clone();
        Some(
            div()
                .mt_auto()
                .mx_3()
                .mb_3()
                .p_2()
                .flex()
                .flex_col()
                .gap_1()
                .bg(colors.raised_background)
                .rounded(px(4.0))
                .child(
                    div()
                        .text_sm()
                        .text_color(colors.warning)
                        .child(format!("● Remote activity: {label}")),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(colors.text_secondary)
                        .child("Running in the background — you can cancel it."),
                )
                .child(crate::views::components::file_action_button(
                    "Cancel network operation",
                    colors,
                    cx,
                    |app, cx| {
                        app.cancel_network_operation(cx);
                    },
                ))
                .into_any_element(),
        )
    }

    #[allow(clippy::unused_self)]
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

    #[allow(clippy::unused_self)]
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
                    .child("\u{1F310}")
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
                    .child("\u{2795}")
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
                        if expanded { "⌄" } else { "›" },
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
                rows.push(
                    div()
                        .id((id_prefix, rows.len()))
                        .h(px(22.0))
                        .px_3()
                        .pl(px(f32::from(indent)))
                        .flex()
                        .items_center()
                        .text_sm()
                        .cursor_pointer()
                        .on_click(cx.listener(move |app, _, _, cx| {
                            app.select_ref_context(context.clone(), cx);
                        }))
                        .child(parts.last().copied().unwrap_or_default().to_owned())
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
        .h(px(24.0))
        .px_3()
        .flex()
        .items_center()
        .gap_2()
        .text_sm()
        .bg(if active {
            colors.selection
        } else {
            colors.sidebar_background
        })
        .rounded(px(4.0))
        .cursor_pointer()
        .on_click(cx.listener(move |app, event, _, cx| on_click(app, event, cx)))
        .child(div().text_xs().text_color(colors.text_muted).child(icon))
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
        .h(px(24.0))
        .px_3()
        .flex()
        .items_center()
        .gap_2()
        .text_sm()
        .bg(if active {
            colors.selection
        } else {
            colors.sidebar_background
        })
        .rounded(px(4.0))
        .cursor_pointer()
        .on_click(cx.listener(move |app, event, _, cx| on_click(app, event, cx)))
        .child(div().text_xs().text_color(colors.text_muted).child(icon))
        .child(label)
        .children(badge.map(|text| {
            div()
                .ml_auto()
                .px_1p5()
                .py_0p5()
                .rounded(px(4.0))
                .bg(colors.accent)
                .text_xs()
                .text_color(colors.panel_background)
                .child(text)
                .into_any_element()
        }))
        .into_any_element()
}

fn welcome_sidebar_view(
    app: &GitronimoApp,
    width: f32,
    colors: &ThemeColors,
    cx: &mut gpui::Context<GitronimoApp>,
) -> AnyElement {
    let mut repositories = Vec::new();
    for (index, path) in app.recents.iter().enumerate() {
        let path = path.clone();
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("Repository")
            .to_owned();
        let selected = app.selected_recent == Some(index);
        repositories.push(
            div()
                .id(("welcome-repository", index))
                .h(px(24.0))
                .px_3()
                .flex()
                .items_center()
                .gap_2()
                .text_sm()
                .bg(if selected {
                    colors.selection
                } else {
                    colors.sidebar_background
                })
                .rounded(px(4.0))
                .cursor_pointer()
                .on_click(cx.listener(move |app, event: &ClickEvent, window, cx| {
                    app.select_recent(index, cx);
                    if event.click_count() >= 2 {
                        app.open_recent(path.clone(), window, cx);
                    }
                }))
                .child("📁")
                .child(name)
                .into_any_element(),
        );
    }
    div()
        .w(px(width))
        .h_full()
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
                .child("Repositories"),
        )
        .child(
            div()
                .px_3()
                .flex()
                .flex_col()
                .gap_0p5()
                .children(repositories),
        )
        .child(
            div()
                .mt_auto()
                .p_3()
                .flex()
                .gap_1()
                .child(
                    div()
                        .id("welcome-add-repository")
                        .px_2()
                        .py_1()
                        .bg(colors.raised_background)
                        .rounded(px(4.0))
                        .cursor_pointer()
                        .on_click(cx.listener(|_, _, window, cx| {
                            window.dispatch_action(Box::new(OpenRepository), cx);
                        }))
                        .child("Add")
                        .into_any_element(),
                )
                .child(crate::views::components::file_action_button(
                    "Create",
                    colors,
                    cx,
                    |app, cx| {
                        app.prompt_create_repository(cx);
                    },
                ))
                .child(crate::views::components::file_action_button(
                    "Clone",
                    colors,
                    cx,
                    |app, cx| {
                        app.prompt_clone_repository(cx);
                    },
                )),
        )
        .into_any_element()
}
