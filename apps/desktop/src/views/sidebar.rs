//! Left sidebar: workspace navigation, status badges, and ref trees.

use gpui::{AnyElement, div, prelude::*, px};
use ui_kit::ThemeColors;

use git_domain::{HeadStatus, NamedRef};

use crate::app_state::{GitronimoApp, RefContext, RefKind};
use crate::views::components::file_action_button;

impl GitronimoApp {
    #[allow(clippy::too_many_lines)]
    pub(crate) fn sidebar_view(
        &self,
        width: f32,
        colors: &ThemeColors,
        cx: &mut gpui::Context<Self>,
    ) -> AnyElement {
        if matches!(self.state, crate::app_state::ShellState::Welcome) {
            return welcome_sidebar_view(width, colors);
        }
        let groups = self.status_groups();
        let branch = self.working_copy.as_ref().map_or_else(
            || "Branch: loading…".to_owned(),
            |status| match &status.branch.head {
                HeadStatus::Branch(name) => format!("Branch: {}", String::from_utf8_lossy(&name.0)),
                HeadStatus::Detached => "Branch: detached HEAD".into(),
                HeadStatus::Unborn => "Branch: unborn".into(),
                HeadStatus::Unknown => "Branch: unknown".into(),
            },
        );
        let upstream = self.working_copy.as_ref().and_then(|status| {
            status.branch.upstream.as_ref().map(|upstream| {
                format!(
                    "Upstream: {} (+{}/-{})",
                    String::from_utf8_lossy(&upstream.0),
                    status.branch.ahead,
                    status.branch.behind
                )
            })
        });
        div()
            .w(px(width))
            .h_full()
            .p_4()
            .flex()
            .flex_col()
            .gap_2()
            .bg(colors.sidebar_background)
            .border_r_1()
            .border_color(colors.border)
            .child("Workspace")
            .child(branch)
            .children(upstream)
            .child(
                div()
                    .id("sidebar-working-copy")
                    .cursor_pointer()
                    .on_click(cx.listener(|app, _, _, cx| {
                        app.navigate_to(crate::app_state::RepositoryView::WorkingCopy, cx);
                    }))
                    .child("Working Copy"),
            )
            .child(status_badge("Staged", groups.staged.len(), colors))
            .child(status_badge("Unstaged", groups.unstaged.len(), colors))
            .child(status_badge("Untracked", groups.untracked.len(), colors))
            .child(status_badge("Conflicts", groups.conflicts.len(), colors))
            .child(
                div()
                    .id("sidebar-history")
                    .cursor_pointer()
                    .on_click(cx.listener(|app, _, _, cx| {
                        app.navigate_to(crate::app_state::RepositoryView::History, cx);
                    }))
                    .child("History"),
            )
            .child(
                div()
                    .id("sidebar-stashes")
                    .flex()
                    .justify_between()
                    .cursor_pointer()
                    .on_click(cx.listener(|app, _, _, cx| {
                        if let crate::app_state::ShellState::Repository(repository) = &app.state {
                            app.show_stashes(repository.clone(), cx);
                        }
                    }))
                    .child("Stashes")
                    .child(self.stashes.len().to_string()),
            )
            .child("Local branches")
            .children(self.ref_rows(
                "local",
                &self.refs.local_branches,
                RefKind::LocalBranch,
                colors,
                cx,
            ))
            .child("Remote branches")
            .children(self.ref_rows(
                "remote",
                &self.refs.remote_branches,
                RefKind::RemoteBranch,
                colors,
                cx,
            ))
            .child("Tags")
            .children(self.ref_rows("tag", &self.refs.tags, RefKind::Tag, colors, cx))
            .child(
                div()
                    .id("sidebar-remotes")
                    .flex()
                    .justify_between()
                    .cursor_pointer()
                    .on_click(cx.listener(|app, _, _, cx| {
                        app.show_remotes(cx);
                    }))
                    .child("Remotes")
                    .child(self.refs.remotes.len().to_string()),
            )
            .child(
                div()
                    .id("sidebar-lfs")
                    .flex()
                    .justify_between()
                    .cursor_pointer()
                    .on_click(cx.listener(|app, _, _, cx| {
                        if let crate::app_state::ShellState::Repository(repository) = &app.state {
                            app.show_lfs(repository.clone(), cx);
                        }
                    }))
                    .child("Git LFS")
                    .child(self.lfs.len().to_string()),
            )
            .child(
                div()
                    .id("sidebar-services")
                    .cursor_pointer()
                    .on_click(cx.listener(|app, _, _, cx| {
                        app.show_services(cx);
                    }))
                    .child("Services"),
            )
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
                                .pl_2()
                                .cursor_pointer()
                                .on_click(cx.listener(move |app, _, _, cx| {
                                    app.select_ref_context(context.clone(), cx);
                                }))
                                .child(name)
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
                .p_2()
                .flex()
                .flex_col()
                .gap_1()
                .bg(colors.raised_background)
                .border_1()
                .border_color(colors.border)
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
                .child(file_action_button(
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
                        .pl(px(f32::from(indent)))
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

fn status_badge(label: &'static str, count: usize, colors: &ThemeColors) -> AnyElement {
    div()
        .flex()
        .justify_between()
        .text_color(colors.text_secondary)
        .child(label)
        .child(count.to_string())
        .into_any_element()
}

fn welcome_sidebar_view(width: f32, colors: &ThemeColors) -> AnyElement {
    div()
        .w(px(width))
        .h_full()
        .p_4()
        .flex()
        .flex_col()
        .gap_3()
        .bg(colors.sidebar_background)
        .border_r_1()
        .border_color(colors.border)
        .child(div().text_lg().child("Workspace"))
        .child(
            div()
                .text_color(colors.text_secondary)
                .child("Open a repository to start reviewing changes, history, and remotes."),
        )
        .child(
            div()
                .mt_4()
                .text_color(colors.text_muted)
                .child("Quick start"),
        )
        .child(
            div()
                .p_3()
                .bg(colors.raised_background)
                .border_1()
                .border_color(colors.border)
                .child("Open a local repository")
                .child(
                    div()
                        .mt_1()
                        .text_sm()
                        .text_color(colors.text_secondary)
                        .child("Command-O or drag a folder into this window"),
                ),
        )
        .child(
            div()
                .mt_4()
                .text_color(colors.text_muted)
                .child("Available here"),
        )
        .child("Working copy and file diffs")
        .child("History and local branches")
        .child("Configured remotes")
        .into_any_element()
}
