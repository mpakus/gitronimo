//! Working Copy view: status groups, changes, sync, branch controls, confirmations.

use gpui::{AnyElement, ClickEvent, MouseButton, div, prelude::*};
use ui_kit::ThemeColors;

use git_domain::{StatusEntry, WorktreeRepository};

use crate::app_state::{
    ForcePushState, GitronimoApp, Mutation, RefContext, RepositoryView, StashAction,
};
use crate::views::components::{
    empty_status_message, file_action_button, mutation_button, state_panel, status_label,
    status_path, validated_action_button, workspace_section,
};

impl GitronimoApp {
    #[allow(clippy::too_many_lines)]
    pub(crate) fn repository_view(
        &self,
        repository: &WorktreeRepository,
        colors: &ThemeColors,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        if self.repository_view == RepositoryView::History {
            return self.history_view(repository, colors, cx).into_any_element();
        }
        let groups = self.status_groups();
        let has_local_branches = !self.refs.local_branches.is_empty();
        let has_remotes = !self.refs.remotes.is_empty();
        let has_upstream = self.has_upstream();
        let has_attached_branch = self.has_attached_branch();
        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .p_4()
                    .flex()
                    .items_center()
                    .justify_between()
                    .bg(colors.panel_background)
                    .border_1()
                    .border_color(colors.border)
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(div().text_xl().child("Working copy"))
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(colors.text_secondary)
                                    .child(repository.worktree_root.display().to_string()),
                            ),
                    )
                    .child(file_action_button("History", colors, cx, {
                        let repository = repository.clone();
                        move |app, cx| app.show_history(repository.clone(), cx)
                    })),
            )
            .children(self.navigation_controls(colors, cx))
            .child(workspace_section(
                "Branch",
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(div().flex().gap_2().children([
                        validated_action_button(
                            "Checkout branch…",
                            has_local_branches,
                            "No local branches are available.",
                            colors,
                            cx,
                            |_, cx| GitronimoApp::prompt_branch_name(false, cx),
                        ),
                        file_action_button("New branch from HEAD…", colors, cx, |_, cx| {
                            GitronimoApp::prompt_branch_name(true, cx);
                        }),
                    ]))
                    .child(div().flex().gap_2().children([
                        validated_action_button(
                            "Rename current branch…",
                            has_attached_branch,
                            "Checkout a local branch first.",
                            colors,
                            cx,
                            |_, cx| GitronimoApp::prompt_rename_current_branch(cx),
                        ),
                        validated_action_button(
                            "Delete local branch…",
                            has_local_branches,
                            "No local branches are available.",
                            colors,
                            cx,
                            |_, cx| GitronimoApp::prompt_delete_local_branch(cx),
                        ),
                    ])),
                colors,
            ))
            .child(workspace_section(
                "Sync",
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(div().flex().gap_2().children([
                        validated_action_button(
                            "Fetch default remote",
                            has_remotes,
                            "Add a remote before fetching.",
                            colors,
                            cx,
                            GitronimoApp::fetch_default_remote,
                        ),
                        validated_action_button(
                            "Fetch remote…",
                            has_remotes,
                            "Add a remote before fetching.",
                            colors,
                            cx,
                            |_, cx| GitronimoApp::prompt_fetch_remote(cx),
                        ),
                    ]))
                    .child(div().flex().gap_2().children([
                        validated_action_button(
                            "Pull current branch",
                            has_upstream,
                            "Set an upstream before pulling.",
                            colors,
                            cx,
                            GitronimoApp::pull_current,
                        ),
                        validated_action_button(
                            "Push current branch",
                            has_upstream,
                            "Set an upstream before pushing.",
                            colors,
                            cx,
                            GitronimoApp::push_current,
                        ),
                    ]))
                    .child(div().flex().gap_2().children([
                        validated_action_button(
                            "Publish current branch",
                            has_remotes && has_attached_branch,
                            "Checkout a branch and add a remote before publishing.",
                            colors,
                            cx,
                            GitronimoApp::publish_current,
                        ),
                        validated_action_button(
                            "Advanced force-with-lease…",
                            has_upstream,
                            "Set an upstream before force-with-lease is available.",
                            colors,
                            cx,
                            GitronimoApp::request_force_with_lease,
                        ),
                    ]))
                    .children(self.network_cancel_button(colors, cx)),
                colors,
            ))
            .children(self.ref_context_menu_view(colors, cx))
            .children(self.working_copy.as_ref().is_none().then(|| {
                state_panel(
                    "Loading working copy",
                    "Reading status, branches, and remotes in the background.",
                    colors.warning,
                    colors,
                )
            }))
            .child(self.mutation_controls(colors, cx))
            .children(self.discard_confirmation_view(colors, cx))
            .children(self.line_discard_confirmation_view(colors, cx))
            .children(self.stash_pop_confirmation_view(colors, cx))
            .children(self.stash_drop_confirmation_view(colors, cx))
            .children(self.branch_delete_confirmation_view(colors, cx))
            .children(self.force_with_lease_confirmation_view(colors, cx))
            .child(self.commit_composer_view(colors, cx))
            .children(self.context_menu_view(repository, colors, cx))
            .child(self.status_group_view("Staged", &groups.staged, true, colors, cx))
            .child(self.status_group_view("Unstaged", &groups.unstaged, false, colors, cx))
            .child(self.status_group_view("Untracked", &groups.untracked, false, colors, cx))
            .child(self.status_group_view("Conflicts", &groups.conflicts, false, colors, cx))
            .children(self.diff_view(colors, cx))
            .into_any_element()
    }

    pub(crate) fn status_groups(&self) -> crate::views::components::StatusGroups<'_> {
        let mut groups = crate::views::components::StatusGroups::default();
        let Some(status) = &self.working_copy else {
            return groups;
        };
        for entry in &status.entries {
            match entry {
                StatusEntry::Unmerged { .. } => groups.conflicts.push(entry),
                StatusEntry::Untracked(_) => groups.untracked.push(entry),
                StatusEntry::Ignored(_) => {}
                StatusEntry::Ordinary { status, .. } | StatusEntry::Renamed { status, .. } => {
                    if status.0[0] != b'.' {
                        groups.staged.push(entry);
                    }
                    if status.0[1] != b'.' {
                        groups.unstaged.push(entry);
                    }
                }
            }
        }
        groups
    }

    pub(crate) fn ref_context_menu_view(
        &self,
        colors: &ThemeColors,
        cx: &mut gpui::Context<Self>,
    ) -> Option<AnyElement> {
        let context = self.ref_context.clone()?;
        let (title, reference) = match &context {
            RefContext::LocalBranch(name) => ("Local branch", name.clone()),
            RefContext::RemoteBranch(name) => ("Remote branch", name.clone()),
            RefContext::Tag(name) => ("Tag", name.clone()),
            RefContext::Remote(name) => ("Remote", name.clone()),
        };
        let mut menu = div()
            .p_2()
            .flex()
            .flex_col()
            .gap_1()
            .bg(colors.raised_background)
            .border_1()
            .border_color(colors.border)
            .child(format!("{title}: {reference}"));
        match context {
            RefContext::LocalBranch(branch) => {
                let checkout = branch.clone();
                let history = branch.clone();
                menu = menu
                    .child(file_action_button(
                        "Checkout branch",
                        colors,
                        cx,
                        move |app, cx| {
                            app.checkout_branch(checkout.clone(), cx);
                        },
                    ))
                    .child(file_action_button(
                        "View branch history",
                        colors,
                        cx,
                        move |app, cx| {
                            app.show_ref_history(history.clone(), cx);
                        },
                    ));
            }
            RefContext::RemoteBranch(branch) | RefContext::Tag(branch) => {
                let create_start = branch.clone();
                let history = branch.clone();
                menu = menu
                    .child(file_action_button(
                        "New branch from ref…",
                        colors,
                        cx,
                        move |_, cx| {
                            GitronimoApp::prompt_branch_from_ref(create_start.clone(), cx);
                        },
                    ))
                    .child(file_action_button(
                        "View ref history",
                        colors,
                        cx,
                        move |app, cx| {
                            app.show_ref_history(history.clone(), cx);
                        },
                    ));
            }
            RefContext::Remote(remote) => {
                menu = menu.child(file_action_button(
                    "Fetch this remote",
                    colors,
                    cx,
                    move |app, cx| {
                        app.run_network_command(
                            format!("Fetching {remote}"),
                            vec!["fetch".into(), "--progress".into(), remote.clone().into()],
                            cx,
                        );
                    },
                ));
            }
        }
        Some(menu.into_any_element())
    }

    pub(crate) fn navigation_controls(
        &self,
        colors: &ThemeColors,
        cx: &mut gpui::Context<Self>,
    ) -> Option<AnyElement> {
        (!self.navigation_back.is_empty() || !self.navigation_forward.is_empty()).then(|| {
            div()
                .flex()
                .gap_2()
                .children(self.navigation_back.last().map(|_| {
                    file_action_button("Back", colors, cx, |app, cx| {
                        if let Some(view) = app.navigation_back.pop() {
                            app.navigation_forward.push(app.repository_view);
                            app.repository_view = view;
                            cx.notify();
                        }
                    })
                }))
                .children(self.navigation_forward.last().map(|_| {
                    file_action_button("Forward", colors, cx, |app, cx| {
                        if let Some(view) = app.navigation_forward.pop() {
                            app.navigation_back.push(app.repository_view);
                            app.repository_view = view;
                            cx.notify();
                        }
                    })
                }))
                .into_any_element()
        })
    }

    pub(crate) fn mutation_controls(
        &self,
        colors: &ThemeColors,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let disabled = self.mutation_in_flight;
        workspace_section(
            "Changes",
            div().flex().gap_2().children([
                mutation_button(
                    "Stage selected",
                    disabled,
                    Mutation::StageSelected,
                    colors,
                    cx,
                ),
                mutation_button(
                    "Unstage selected",
                    disabled,
                    Mutation::UnstageSelected,
                    colors,
                    cx,
                ),
                mutation_button("Stage all", disabled, Mutation::StageAll, colors, cx),
                mutation_button("Unstage all", disabled, Mutation::UnstageAll, colors, cx),
                mutation_button(
                    "Discard selected",
                    disabled,
                    Mutation::DiscardSelected,
                    colors,
                    cx,
                ),
                file_action_button("Stash tracked changes", colors, cx, |app, cx| {
                    app.create_stash(false, cx);
                }),
                file_action_button("Stash including untracked", colors, cx, |app, cx| {
                    app.create_stash(true, cx);
                }),
                file_action_button("Apply latest stash", colors, cx, |app, cx| {
                    app.apply_latest_stash(cx);
                }),
                file_action_button("Pop latest stash", colors, cx, |app, cx| {
                    app.pending_stash_action = Some(StashAction::Pop);
                    app.activity =
                        "Confirm before removing the latest stash recovery entry.".into();
                    cx.notify();
                }),
                file_action_button("Drop latest stash", colors, cx, |app, cx| {
                    app.pending_stash_action = Some(StashAction::Drop);
                    app.activity = "Confirm before permanently removing the latest stash.".into();
                    cx.notify();
                }),
            ]),
            colors,
        )
    }

    pub(crate) fn discard_confirmation_view(
        &self,
        colors: &ThemeColors,
        cx: &mut gpui::Context<Self>,
    ) -> Option<AnyElement> {
        self.pending_discard.as_ref().map(|paths| {
            div()
                .p_2()
                .bg(colors.raised_background)
                .border_1()
                .border_color(colors.border)
                .child(format!(
                    "Discard {} path(s)? Tracked changes restore from HEAD; untracked files move to Trash.",
                    paths.len()
                ))
                .child(file_action_button("Confirm discard", colors, cx, |app, cx| {
                    app.confirm_discard(cx);
                }))
                .into_any_element()
        })
    }

    pub(crate) fn line_discard_confirmation_view(
        &self,
        colors: &ThemeColors,
        cx: &mut gpui::Context<Self>,
    ) -> Option<AnyElement> {
        self.pending_line_discard.as_ref().map(|(path, selection)| {
            div()
                .p_2()
                .bg(colors.raised_background)
                .border_1()
                .border_color(colors.border)
                .child(format!(
                    "Discard {} selected line(s) in {}? Their working-copy changes are restored from the index.",
                    selection.len(),
                    String::from_utf8_lossy(&path.0)
                ))
                .child(file_action_button(
                    "Confirm line discard",
                    colors,
                    cx,
                    GitronimoApp::confirm_line_discard,
                ))
                .child(file_action_button(
                    "Cancel line discard",
                    colors,
                    cx,
                    GitronimoApp::cancel_line_discard,
                ))
                .into_any_element()
        })
    }

    pub(crate) fn stash_pop_confirmation_view(
        &self,
        colors: &ThemeColors,
        cx: &mut gpui::Context<Self>,
    ) -> Option<AnyElement> {
        (self.pending_stash_action == Some(StashAction::Pop)).then(|| {
            div()
                .p_2()
                .bg(colors.raised_background)
                .border_1()
                .border_color(colors.border)
                .child("Pop the latest stash? Its recovery entry will be removed after a successful apply.")
                .child(file_action_button("Confirm pop latest stash", colors, cx, |app, cx| {
                    app.pop_latest_stash(cx);
                }))
                .into_any_element()
        })
    }

    pub(crate) fn stash_drop_confirmation_view(
        &self,
        colors: &ThemeColors,
        cx: &mut gpui::Context<Self>,
    ) -> Option<AnyElement> {
        (self.pending_stash_action == Some(StashAction::Drop)).then(|| {
            div()
                .p_2()
                .bg(colors.raised_background)
                .border_1()
                .border_color(colors.border)
                .child("Drop the latest stash permanently? This cannot be undone.")
                .child(file_action_button(
                    "Confirm drop latest stash",
                    colors,
                    cx,
                    GitronimoApp::drop_latest_stash,
                ))
                .into_any_element()
        })
    }

    pub(crate) fn branch_delete_confirmation_view(
        &self,
        colors: &ThemeColors,
        cx: &mut gpui::Context<Self>,
    ) -> Option<AnyElement> {
        self.pending_branch_delete.as_ref().map(|branch| {
            div()
                .p_2()
                .bg(colors.raised_background)
                .border_1()
                .border_color(colors.border)
                .child(format!(
                    "Delete local branch {branch}? Safe deletion refuses unmerged work."
                ))
                .child(file_action_button(
                    "Delete merged branch",
                    colors,
                    cx,
                    |app, cx| {
                        app.confirm_branch_delete(false, cx);
                    },
                ))
                .child(file_action_button(
                    "Force delete unmerged branch",
                    colors,
                    cx,
                    |app, cx| app.confirm_branch_delete(true, cx),
                ))
                .into_any_element()
        })
    }

    pub(crate) fn network_cancel_button(
        &self,
        colors: &ThemeColors,
        cx: &mut gpui::Context<Self>,
    ) -> Option<AnyElement> {
        self.network_operation.as_ref().map(|_| {
            file_action_button("Cancel network operation", colors, cx, |app, cx| {
                app.cancel_network_operation(cx);
            })
            .into_any_element()
        })
    }

    pub(crate) fn force_with_lease_confirmation_view(
        &self,
        colors: &ThemeColors,
        cx: &mut gpui::Context<Self>,
    ) -> Option<AnyElement> {
        (self.force_push_state == ForcePushState::AwaitingConfirmation).then(|| {
            div()
                .p_2()
                .bg(colors.raised_background)
                .border_1()
                .border_color(colors.border)
                .child("Force-with-lease can replace remote commits only when your fetched remote ref is current.")
                .child(file_action_button("Confirm force-with-lease", colors, cx, |app, cx| {
                    app.confirm_force_with_lease(cx);
                }))
                .into_any_element()
        })
    }

    pub(crate) fn context_menu_view(
        &self,
        repository: &WorktreeRepository,
        colors: &ThemeColors,
        cx: &mut gpui::Context<Self>,
    ) -> Option<AnyElement> {
        self.context_path.as_ref().map(|path| {
            let copy_repository = repository.clone();
            let reveal_repository = repository.clone();
            let open_repository = repository.clone();
            div()
                .p_2()
                .bg(colors.raised_background)
                .border_1()
                .border_color(colors.border)
                .child(format!(
                    "File actions: {}",
                    String::from_utf8_lossy(&path.0)
                ))
                .child(
                    div()
                        .flex()
                        .gap_2()
                        .child(file_action_button(
                            "Copy path",
                            colors,
                            cx,
                            move |app, cx| {
                                app.copy_context_path(&copy_repository, cx);
                            },
                        ))
                        .child(file_action_button(
                            "Reveal in Finder",
                            colors,
                            cx,
                            move |app, cx| {
                                app.open_context_path(&reveal_repository, true, cx);
                            },
                        ))
                        .child(file_action_button(
                            "Open in editor",
                            colors,
                            cx,
                            move |app, cx| {
                                app.open_context_path(&open_repository, false, cx);
                            },
                        )),
                )
                .into_any_element()
        })
    }

    pub(crate) fn status_group_view(
        &self,
        title: &'static str,
        entries: &[&StatusEntry],
        staged: bool,
        colors: &ThemeColors,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let rows = if entries.is_empty() {
            div()
                .text_color(colors.text_muted)
                .child(empty_status_message(title))
                .into_any_element()
        } else {
            div()
                .flex()
                .flex_col()
                .gap_1()
                .children(entries.iter().enumerate().map(|(index, entry)| {
                    let path = status_path(entry).clone();
                    let context_path = path.clone();
                    let selected = self.selected_paths.contains(&path);
                    div()
                        .id((title, index))
                        .px_2()
                        .py_1()
                        .bg(if selected {
                            colors.raised_background
                        } else {
                            colors.panel_background
                        })
                        .border_1()
                        .border_color(colors.border)
                        .cursor_pointer()
                        .on_click(cx.listener(move |app, event: &ClickEvent, _, cx| {
                            app.select_status_path(
                                path.clone(),
                                event.modifiers().secondary() || event.modifiers().shift,
                                staged,
                                cx,
                            );
                        }))
                        .on_mouse_down(
                            MouseButton::Right,
                            cx.listener(move |app, _, _, cx| {
                                app.show_status_context_menu(context_path.clone(), cx);
                            }),
                        )
                        .child(status_label(entry))
                }))
                .into_any_element()
        };
        div()
            .p_3()
            .flex()
            .flex_col()
            .gap_2()
            .bg(colors.panel_background)
            .border_1()
            .border_color(colors.border)
            .child(
                div().flex().justify_between().child(title).child(
                    div()
                        .text_color(colors.text_secondary)
                        .child(entries.len().to_string()),
                ),
            )
            .child(rows)
    }
}
