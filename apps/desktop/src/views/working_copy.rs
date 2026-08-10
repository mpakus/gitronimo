//! Working Copy view: status groups, changes, sync, branch controls, confirmations.

use gpui::{AnyElement, ClickEvent, MouseButton, div, prelude::*, px};
use ui_kit::ThemeColors;

use git_domain::{GitPath, InProgressOperation, StatusEntry, WorktreeRepository};

use crate::app_state::{
    ForcePushState, GitronimoApp, Mutation, OperationAction, RefContext, RepositoryView,
    StashAction,
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
        if self.repository_view == RepositoryView::CommitDetail {
            return self
                .commit_detail_view(repository, colors, cx)
                .into_any_element();
        }
        if self.repository_view == RepositoryView::Stashes {
            return self.stashes_view(colors, cx).into_any_element();
        }
        if self.repository_view == RepositoryView::Remotes {
            return self.remotes_view(colors, cx).into_any_element();
        }
        if self.repository_view == RepositoryView::Reflog {
            return self.reflog_view(repository, colors, cx).into_any_element();
        }
        if self.repository_view == RepositoryView::FileHistory {
            return self
                .file_history_view(repository, colors, cx)
                .into_any_element();
        }
        if self.repository_view == RepositoryView::Blame {
            return self.blame_view(repository, colors, cx).into_any_element();
        }
        if self.repository_view == RepositoryView::Compare {
            return self.compare_view(repository, colors, cx).into_any_element();
        }
        if self.repository_view == RepositoryView::Tree {
            return self.tree_view(repository, colors, cx).into_any_element();
        }
        if self.repository_view == RepositoryView::Worktrees {
            return self
                .worktrees_view(repository, colors, cx)
                .into_any_element();
        }
        if self.repository_view == RepositoryView::Submodules {
            return self
                .submodules_view(repository, colors, cx)
                .into_any_element();
        }
        if self.repository_view == RepositoryView::Lfs {
            return self.lfs_view(colors, cx).into_any_element();
        }
        if self.repository_view == RepositoryView::Rebase {
            return self.rebase_view(repository, colors, cx).into_any_element();
        }
        if self.repository_view == RepositoryView::Conflicts {
            return self
                .conflicts_view(repository, colors, cx)
                .into_any_element();
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
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(file_action_button(
                                if self.worktree_show_all_files {
                                    "Modified only"
                                } else {
                                    "All files"
                                },
                                colors,
                                cx,
                                super::super::app_state::GitronimoApp::toggle_worktree_show_all,
                            ))
                            .child(file_action_button("History", colors, cx, {
                                let repository = repository.clone();
                                move |app, cx| app.show_history(repository.clone(), cx)
                            })),
                    ),
            )
            .children(self.navigation_controls(colors, cx))
            .children(self.operation_banner_view(colors, cx))
            .children(self.operation_confirmation_view(colors, cx))
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
            .children(self.hunk_discard_confirmation_view(colors, cx))
            .children(self.stash_pop_confirmation_view(colors, cx))
            .children(self.stash_drop_confirmation_view(colors, cx))
            .children(self.branch_delete_confirmation_view(colors, cx))
            .children(self.force_with_lease_confirmation_view(colors, cx))
            .children(self.context_menu_view(repository, colors, cx))
            .child(self.commit_composer_view(colors, cx))
            .when(self.worktree_show_all_files, |this| {
                this.child(self.all_files_group_view(colors, cx))
            })
            .when(!self.worktree_show_all_files, |this| {
                this.child(self.status_group_view("Staged", &groups.staged, true, colors, cx))
                    .child(self.status_group_view("Unstaged", &groups.unstaged, false, colors, cx))
                    .child(self.status_group_view(
                        "Untracked",
                        &groups.untracked,
                        false,
                        colors,
                        cx,
                    ))
                    .child(self.status_group_view(
                        "Conflicts",
                        &groups.conflicts,
                        false,
                        colors,
                        cx,
                    ))
            })
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

    pub(crate) fn operation_banner_view(
        &self,
        colors: &ThemeColors,
        cx: &mut gpui::Context<Self>,
    ) -> Option<AnyElement> {
        let operation = self.working_copy.as_ref()?.operation.clone();
        (operation != InProgressOperation::None).then(|| {
            let (title, detail) = operation_description(&operation);
            let conflict_count = self.status_groups().conflicts.len();
            let overview = operation_conflict_overview(conflict_count);
            div()
                .p_4()
                .flex()
                .flex_col()
                .gap_2()
                .bg(colors.panel_background)
                .border_1()
                .border_color(colors.warning)
                .child(div().text_color(colors.warning).child(title))
                .child(div().text_color(colors.text_secondary).child(detail))
                .child(div().text_color(colors.text_secondary).child(overview))
                .child(
                    div()
                        .flex()
                        .gap_2()
                        .child(file_action_button(
                            "Abort operation",
                            colors,
                            cx,
                            GitronimoApp::request_operation_abort,
                        ))
                        .child(file_action_button(
                            "Continue operation",
                            colors,
                            cx,
                            GitronimoApp::request_operation_continue,
                        )),
                )
                .into_any_element()
        })
    }

    pub(crate) fn operation_confirmation_view(
        &self,
        colors: &ThemeColors,
        cx: &mut gpui::Context<Self>,
    ) -> Option<AnyElement> {
        self.pending_operation_action.map(|action| {
            let (confirm_label, cancel_label, message) = match action {
                OperationAction::Abort => (
                    "Confirm abort",
                    "Cancel abort",
                    "Abort this operation? Its conflict work is discarded and the repository returns to the operation start state.",
                ),
                OperationAction::Continue => (
                    "Confirm continue",
                    "Cancel continue",
                    "Continue this operation? The staged conflict resolution is committed and the operation finishes.",
                ),
            };
            div()
                .p_2()
                .bg(colors.raised_background)
                .border_1()
                .border_color(colors.border)
                .child(message.to_owned())
                .child(file_action_button(confirm_label, colors, cx, |app, cx| {
                    app.confirm_operation_action(cx);
                }))
                .child(file_action_button(cancel_label, colors, cx, |app, cx| {
                    app.cancel_operation_action(cx);
                }))
                .into_any_element()
        })
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

    pub(crate) fn hunk_discard_confirmation_view(
        &self,
        colors: &ThemeColors,
        cx: &mut gpui::Context<Self>,
    ) -> Option<AnyElement> {
        self.pending_hunk_discard
            .as_ref()
            .map(|(path, hunk_index)| {
                div()
                .p_2()
                .bg(colors.raised_background)
                .border_1()
                .border_color(colors.border)
                .child(format!(
                    "Discard hunk {} in {}? Its working-copy changes are restored from the index.",
                    hunk_index + 1,
                    String::from_utf8_lossy(&path.0)
                ))
                .child(file_action_button(
                    "Confirm hunk discard",
                    colors,
                    cx,
                    GitronimoApp::confirm_hunk_discard,
                ))
                .child(file_action_button(
                    "Cancel hunk discard",
                    colors,
                    cx,
                    GitronimoApp::cancel_hunk_discard,
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
                    self.status_row(
                        (title, index).into(),
                        status_path(entry).clone(),
                        status_label(entry),
                        staged,
                        colors,
                        cx,
                    )
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

    pub(crate) fn all_files_group_view(
        &self,
        colors: &ThemeColors,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let groups = self.status_groups();
        let staged: std::collections::HashSet<GitPath> = groups
            .staged
            .iter()
            .map(|entry| status_path(entry).clone())
            .collect();
        let unstaged: std::collections::HashSet<GitPath> = groups
            .unstaged
            .iter()
            .map(|entry| status_path(entry).clone())
            .collect();
        let conflicts: std::collections::HashSet<GitPath> = groups
            .conflicts
            .iter()
            .map(|entry| status_path(entry).clone())
            .collect();
        let mut rows: Vec<AnyElement> = Vec::new();
        for (index, path) in self.tracked_files.iter().enumerate() {
            let display = String::from_utf8_lossy(&path.0);
            let (label, staged) = if conflicts.contains(path) {
                (format!("UU  {display}"), false)
            } else if staged.contains(path) {
                (format!("M   {display}"), true)
            } else if unstaged.contains(path) {
                (format!(" M  {display}"), false)
            } else {
                (format!("    {display}"), false)
            };
            rows.push(self.status_row(
                ("all-file", index).into(),
                path.clone(),
                label,
                staged,
                colors,
                cx,
            ));
        }
        for (index, entry) in groups.untracked.iter().enumerate() {
            let path = status_path(entry).clone();
            rows.push(self.status_row(
                ("all-untracked", index).into(),
                path.clone(),
                format!("??  {}", String::from_utf8_lossy(&path.0)),
                false,
                colors,
                cx,
            ));
        }
        let count = self.tracked_files.len() + groups.untracked.len();
        let body = if self.tracked_files.is_empty() && groups.untracked.is_empty() {
            div()
                .text_color(colors.text_muted)
                .child("No tracked files to list.")
                .into_any_element()
        } else {
            div()
                .flex()
                .flex_col()
                .gap_1()
                .children(rows)
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
                div().flex().justify_between().child("All files").child(
                    div()
                        .text_color(colors.text_secondary)
                        .child(count.to_string()),
                ),
            )
            .child(body)
    }

    pub(crate) fn status_row(
        &self,
        id: gpui::ElementId,
        path: GitPath,
        label: String,
        staged: bool,
        colors: &ThemeColors,
        cx: &mut gpui::Context<Self>,
    ) -> AnyElement {
        let context_path = path.clone();
        let checkbox_path = path.clone();
        let checkbox_id = id.clone();
        let selected = self.selected_paths.contains(&path);
        div()
            .id(id)
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
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .id(gpui::ElementId::from((checkbox_id, label.clone())))
                            .w(px(16.0))
                            .h(px(16.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .border_1()
                            .border_color(if staged {
                                colors.success
                            } else {
                                colors.border
                            })
                            .text_color(if staged {
                                colors.success
                            } else {
                                colors.text_muted
                            })
                            .cursor_pointer()
                            .on_click(cx.listener(move |app, _: &ClickEvent, _, cx| {
                                cx.stop_propagation();
                                app.toggle_path_staged(checkbox_path.clone(), staged, cx);
                            }))
                            .child(if staged { "✓" } else { "○" }),
                    )
                    .child(div().child(label)),
            )
            .into_any_element()
    }
}

/// The conflict-overview line shown inside the operation banner.
pub(crate) fn operation_conflict_overview(conflict_count: usize) -> String {
    if conflict_count > 0 {
        format!(
            "{conflict_count} conflicted file(s) below — resolve each, stage it, then Continue."
        )
    } else {
        "No conflicted files — stage your changes, then Continue.".into()
    }
}

fn operation_description(operation: &InProgressOperation) -> (String, String) {
    match operation {
        InProgressOperation::Merge { oid } => (
            "Merge in progress".into(),
            format!(
                "Target {} — resolve conflicts, stage the resolved files, then Continue or Abort.",
                short_oid(oid.as_deref())
            ),
        ),
        InProgressOperation::CherryPick { oid } => (
            "Cherry-pick in progress".into(),
            format!(
                "Commit {} — resolve conflicts, stage the resolved files, then Continue or Abort.",
                short_oid(oid.as_deref())
            ),
        ),
        InProgressOperation::Revert { oid } => (
            "Revert in progress".into(),
            format!(
                "Commit {} — resolve conflicts, stage the resolved files, then Continue or Abort.",
                short_oid(oid.as_deref())
            ),
        ),
        InProgressOperation::Rebase => (
            "Rebase in progress".into(),
            "Resolve conflicts, stage the resolved files, then Continue or Abort.".into(),
        ),
        InProgressOperation::None => ("No operation in progress".into(), String::new()),
    }
}

fn short_oid(oid: Option<&[u8]>) -> String {
    oid.and_then(|oid| std::str::from_utf8(oid).ok())
        .map(|oid| oid.chars().take(7).collect::<String>())
        .filter(|short| !short.is_empty())
        .unwrap_or_else(|| "an unknown commit".into())
}
