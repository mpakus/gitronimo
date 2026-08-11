//! Working Copy view: status groups, changes, sync, branch controls, confirmations.

use gpui::{AnyElement, ClickEvent, MouseButton, div, prelude::*, px};
use ui_kit::ThemeColors;

use git_domain::{GitPath, HeadStatus, InProgressOperation, StatusEntry, WorktreeRepository};

use crate::app_state::{
    ForcePushState, GitronimoApp, Mutation, OperationAction, RefContext, RepositoryView,
    StashAction,
};
use crate::views::components::{
    centered_empty_state, file_action_button, list_pane_resize_handle, mutation_button,
    state_panel, status_badge_info, status_badge_square, status_label, status_path,
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
        if self.repository_view == RepositoryView::Services {
            return self.services_view(colors, cx).into_any_element();
        }
        if self.repository_view == RepositoryView::Settings {
            return self.settings_view(colors, cx).into_any_element();
        }
        if self.repository_view == RepositoryView::PullRequests {
            return self.pull_requests_view(colors, cx).into_any_element();
        }
        if self.repository_view == RepositoryView::BranchesReview {
            return self.branches_review_view(colors, cx).into_any_element();
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
        div()
            .flex()
            .flex_col()
            .flex_1()
            .h_full()
            .overflow_hidden()
            .children(self.navigation_controls(colors, cx))
            .children(self.operation_banner_view(colors, cx))
            .children(self.operation_confirmation_view(colors, cx))
            .children(self.ref_context_menu_view(colors, cx))
            .children(self.working_copy.as_ref().is_none().then(|| {
                state_panel(
                    "Loading working copy",
                    "Reading status, branches, and remotes in the background.",
                    colors.warning,
                    colors,
                )
            }))
            .children(self.discard_confirmation_view(colors, cx))
            .children(self.line_discard_confirmation_view(colors, cx))
            .children(self.hunk_discard_confirmation_view(colors, cx))
            .children(self.stash_pop_confirmation_view(colors, cx))
            .children(self.stash_drop_confirmation_view(colors, cx))
            .children(self.branch_delete_confirmation_view(colors, cx))
            .children(self.force_with_lease_confirmation_view(colors, cx))
            .children(self.context_menu_view(repository, colors, cx))
            .child(self.file_review_workspace(colors, cx))
            .into_any_element()
    }

    fn branch_context_view(
        &self,
        _has_local_branches: bool,
        colors: &ThemeColors,
        _cx: &mut gpui::Context<Self>,
    ) -> AnyElement {
        let (branch, tracking) = self.working_copy.as_ref().map_or_else(
            || ("Branch loading…".to_owned(), String::new()),
            |status| {
                let branch = match &status.branch.head {
                    HeadStatus::Branch(name) => String::from_utf8_lossy(&name.0).into_owned(),
                    HeadStatus::Detached => "Detached HEAD".into(),
                    HeadStatus::Unborn => "Unborn branch".into(),
                    HeadStatus::Unknown => "Unknown branch".into(),
                };
                let tracking = status.branch.upstream.as_ref().map_or_else(
                    || "No Tracking".to_owned(),
                    |upstream| String::from_utf8_lossy(&upstream.0).into_owned(),
                );
                (branch, tracking)
            },
        );
        div()
            .h(px(32.0))
            .px_3()
            .flex()
            .items_center()
            .gap_1()
            .border_b_1()
            .border_color(colors.border)
            .child(
                div()
                    .text_xs()
                    .text_color(colors.accent)
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .child("\u{2387}"),
            )
            .child(
                div()
                    .text_sm()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(colors.text_primary)
                    .child(branch),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(colors.text_muted)
                    .child("\u{203A}"),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(colors.text_secondary)
                    .child(tracking),
            )
            .into_any_element()
    }

    fn file_review_workspace(
        &self,
        colors: &ThemeColors,
        cx: &mut gpui::Context<Self>,
    ) -> AnyElement {
        let groups = self.status_groups();
        let modified_count = groups.staged.len()
            + groups.unstaged.len()
            + groups.untracked.len()
            + groups.conflicts.len();
        let file_groups = div()
            .flex()
            .flex_col()
            .child(self.file_list_header(modified_count, colors, cx))
            .when(self.worktree_show_all_files, |this| {
                this.child(self.all_files_group_view(colors, cx))
            })
            .when(!self.worktree_show_all_files, |this| {
                this.child(self.modified_files_list_view(colors, cx))
            });
        let file_list = div().flex_1().overflow_hidden().child(file_groups);
        let diff = self.diff_view(colors, cx).unwrap_or_else(|| {
            let (title, detail) = if self.selected_paths.len() > 1 {
                (
                    "Multiple files selected",
                    "Select one file to inspect its diff.",
                )
            } else {
                (
                    "No file selected",
                    "Choose a changed file to inspect its diff.",
                )
            };
            centered_empty_state(title, detail, colors)
        });
        let diff_pane = div().flex_1().h_full().overflow_hidden().child(diff);
        let col_w = px(self.column_width);
        div()
            .id("workspace-flex")
            .flex()
            .flex_1()
            .h_full()
            .child(
                div()
                    .w(col_w)
                    .min_w(px(crate::app_state::MINIMUM_LIST_PANE_WIDTH))
                    .max_w(px(crate::app_state::MAXIMUM_LIST_PANE_WIDTH))
                    .h_full()
                    .flex()
                    .flex_col()
                    .overflow_hidden()
                    .child(self.branch_context_view(false, colors, cx))
                    .child(self.commit_composer_view(colors, cx))
                    .child(file_list),
            )
            .child(list_pane_resize_handle(self.column_width, colors, cx))
            .child(diff_pane)
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

    fn modified_entries(&self) -> Vec<&StatusEntry> {
        let Some(status) = &self.working_copy else {
            return Vec::new();
        };
        let search = self.worktree_file_search.to_lowercase();
        status
            .entries
            .iter()
            .filter(|entry| match entry {
                StatusEntry::Untracked(_) | StatusEntry::Unmerged { .. } => true,
                StatusEntry::Ordinary { status, .. } | StatusEntry::Renamed { status, .. } => {
                    status.0[0] != b'.' || status.0[1] != b'.'
                }
                StatusEntry::Ignored(_) => false,
            })
            .filter(|entry| {
                search.is_empty()
                    || String::from_utf8_lossy(&status_path(entry).0)
                        .to_lowercase()
                        .contains(&search)
            })
            .collect()
    }

    fn file_list_column_header(colors: &ThemeColors) -> impl IntoElement {
        div()
            .h(px(22.0))
            .px_2()
            .flex()
            .items_center()
            .text_xs()
            .text_color(colors.text_muted)
            .border_b_1()
            .border_color(colors.border)
            .child(div().w(px(44.0)).child("Status"))
            .child(div().flex_1().child("Filename"))
    }

    fn file_list_header(
        &self,
        modified_count: usize,
        colors: &ThemeColors,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let show_all = self.worktree_show_all_files;
        div()
            .h(px(28.0))
            .px_2()
            .flex()
            .items_center()
            .justify_between()
            .border_b_1()
            .border_color(colors.border)
            .child(
                div()
                    .flex()
                    .p_0p5()
                    .bg(colors.raised_background)
                    .rounded(px(4.0))
                    .child(file_list_mode_tab(
                        "Modified",
                        !show_all,
                        colors,
                        cx,
                        |app, cx| {
                            if app.worktree_show_all_files {
                                app.toggle_worktree_show_all(cx);
                            }
                        },
                    ))
                    .child(file_list_mode_tab(
                        "All Files",
                        show_all,
                        colors,
                        cx,
                        |app, cx| {
                            if !app.worktree_show_all_files {
                                app.toggle_worktree_show_all(cx);
                            }
                        },
                    )),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(colors.text_muted)
                    .child(if show_all {
                        format!("{} tracked", self.tracked_files.len())
                    } else if modified_count == 0 {
                        "No changes".into()
                    } else {
                        format!("{modified_count} changed")
                    }),
            )
    }

    fn modified_files_list_view(
        &self,
        colors: &ThemeColors,
        cx: &mut gpui::Context<Self>,
    ) -> AnyElement {
        let entries = self.modified_entries();
        if entries.is_empty() {
            return centered_empty_state(
                "Working tree clean",
                "Edit files in your editor and changes appear here.",
                colors,
            );
        }
        let rows: Vec<AnyElement> = entries
            .iter()
            .enumerate()
            .map(|(index, entry)| {
                let path = status_path(entry).clone();
                let staged = entry_is_staged(entry);
                self.status_row(
                    ("modified-file", index).into(),
                    path,
                    status_label(entry),
                    staged,
                    colors,
                    cx,
                )
            })
            .collect();
        div()
            .flex()
            .flex_col()
            .child(Self::file_list_column_header(colors))
            .children(rows)
            .into_any_element()
    }

    #[allow(clippy::too_many_lines)]
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
            .min_w(px(200.0))
            .bg(colors.raised_background)
            .border_1()
            .border_color(colors.border)
            .rounded(px(4.0))
            .child(
                div()
                    .px_2()
                    .py_1()
                    .text_xs()
                    .text_color(colors.text_muted)
                    .border_b_1()
                    .border_color(colors.border)
                    .child(format!("{title}: {reference}")),
            );
        match context {
            RefContext::LocalBranch(branch) => {
                let checkout = branch.clone();
                let history = branch.clone();
                let delete_branch = branch.clone();
                let rename_branch = branch.clone();
                let merge_branch = branch.clone();
                let rebase_branch = branch.clone();
                menu = menu
                    .child(menu_item("Checkout", colors, cx, move |app, _, cx| {
                        app.checkout_branch(checkout.clone(), cx);
                    }))
                    .child(menu_item("View History", colors, cx, move |app, _, cx| {
                        app.show_ref_history(history.clone(), cx);
                    }))
                    .child(menu_separator(colors))
                    .child(menu_item(
                        "Merge into Current…",
                        colors,
                        cx,
                        move |app, _, cx| {
                            app.merge_branch_into_current(merge_branch.clone(), cx);
                        },
                    ))
                    .child(menu_item(
                        "Rebase Current onto…",
                        colors,
                        cx,
                        move |app, _, cx| {
                            app.rebase_current_onto(rebase_branch.clone(), cx);
                        },
                    ))
                    .child(menu_separator(colors))
                    .child(menu_item("Rename…", colors, cx, move |app, _, cx| {
                        app.prompt_rename_branch(rename_branch.clone(), cx);
                    }))
                    .child(menu_item("Delete", colors, cx, move |app, _, cx| {
                        app.request_branch_delete(delete_branch.clone(), cx);
                    }));
            }
            RefContext::RemoteBranch(branch) => {
                let create_start = branch.clone();
                let history = branch.clone();
                let pull_branch = branch.clone();
                let delete_ref = branch.clone();
                menu = menu
                    .child(menu_item(
                        "New Branch from Here…",
                        colors,
                        cx,
                        move |app, _, cx| {
                            app.prompt_create_branch_from_ref(create_start.clone(), cx);
                        },
                    ))
                    .child(menu_item("View History", colors, cx, move |app, _, cx| {
                        app.show_ref_history(history.clone(), cx);
                    }))
                    .child(menu_separator(colors))
                    .child(menu_item("Pull", colors, cx, move |app, _, cx| {
                        app.pull_branch(pull_branch.clone(), cx);
                    }))
                    .child(menu_separator(colors))
                    .child(menu_item("Delete", colors, cx, move |app, _, cx| {
                        app.request_branch_delete(delete_ref.clone(), cx);
                    }));
            }
            RefContext::Tag(branch) => {
                let create_start = branch.clone();
                let history = branch.clone();
                let delete_ref = branch.clone();
                menu = menu
                    .child(menu_item(
                        "New Branch from Here…",
                        colors,
                        cx,
                        move |app, _, cx| {
                            app.prompt_create_branch_from_ref(create_start.clone(), cx);
                        },
                    ))
                    .child(menu_item("View History", colors, cx, move |app, _, cx| {
                        app.show_ref_history(history.clone(), cx);
                    }))
                    .child(menu_separator(colors))
                    .child(menu_item("Delete", colors, cx, move |app, _, cx| {
                        app.request_branch_delete(delete_ref.clone(), cx);
                    }));
            }
            RefContext::Remote(remote) => {
                let fetch_remote = remote.clone();
                menu = menu.child(menu_item("Fetch", colors, cx, move |app, _, cx| {
                    app.run_network_command(
                        format!("Fetching {fetch_remote}"),
                        vec![
                            "fetch".into(),
                            "--progress".into(),
                            fetch_remote.clone().into(),
                        ],
                        cx,
                    );
                }));
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

    #[allow(dead_code)]
    pub(crate) fn mutation_controls(
        &self,
        colors: &ThemeColors,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let disabled = self.mutation_in_flight;
        div()
            .flex()
            .flex_wrap()
            .gap_1()
            .child(mutation_button(
                "Stage selected",
                disabled,
                Mutation::StageSelected,
                colors,
                cx,
            ))
            .child(mutation_button(
                "Unstage selected",
                disabled,
                Mutation::UnstageSelected,
                colors,
                cx,
            ))
            .child(mutation_button(
                "Stage all",
                disabled,
                Mutation::StageAll,
                colors,
                cx,
            ))
            .child(mutation_button(
                "Unstage all",
                disabled,
                Mutation::UnstageAll,
                colors,
                cx,
            ))
            .child(mutation_button(
                "Discard selected",
                disabled,
                Mutation::DiscardSelected,
                colors,
                cx,
            ))
            .child(file_action_button("Stash", colors, cx, |app, cx| {
                app.create_stash(false, cx);
            }))
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

    #[allow(dead_code)]
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
        let search = self.worktree_file_search.to_lowercase();
        let mut rows: Vec<AnyElement> = Vec::new();
        for (index, path) in self.tracked_files.iter().enumerate() {
            let display = String::from_utf8_lossy(&path.0);
            if !search.is_empty() && !display.to_lowercase().contains(&search) {
                continue;
            }
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
            let display = String::from_utf8_lossy(&path.0);
            if !search.is_empty() && !display.to_lowercase().contains(&search) {
                continue;
            }
            rows.push(self.status_row(
                ("all-untracked", index).into(),
                path.clone(),
                format!("??  {}", String::from_utf8_lossy(&path.0)),
                false,
                colors,
                cx,
            ));
        }
        let body = if self.tracked_files.is_empty() && groups.untracked.is_empty() {
            div()
                .text_color(colors.text_muted)
                .child("No tracked files to list.")
                .into_any_element()
        } else {
            div().flex().flex_col().children(rows).into_any_element()
        };
        div()
            .flex()
            .flex_col()
            .child(Self::file_list_column_header(colors))
            .child(body)
    }

    #[allow(
        clippy::similar_names,
        clippy::needless_pass_by_value,
        clippy::too_many_lines
    )]
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
        let (badge_char, badge_bg, _) = status_badge_info(&label, colors);
        let badge_char = badge_char.to_owned();
        let display_path = label
            .trim_start_matches(|c: char| c.is_alphabetic() || c == ' ')
            .trim_start();

        // Create the checkbox first
        let checkbox = {
            let mut checkbox = div()
                .id(gpui::ElementId::from((checkbox_id, label.clone())))
                .w(px(14.0))
                .h(px(14.0))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(3.0))
                .bg(if staged {
                    colors.accent
                } else {
                    colors.panel_background
                })
                .border_1()
                .border_color(if staged { colors.accent } else { colors.border })
                .text_color(if staged {
                    colors.panel_background
                } else {
                    colors.text_muted
                })
                .cursor_pointer();
            checkbox
                .interactivity()
                .on_click(cx.listener(move |app, _: &ClickEvent, _, cx| {
                    cx.stop_propagation();
                    app.toggle_path_staged(checkbox_path.clone(), staged, cx);
                }));
            checkbox
                .child(if staged { "\u{2713}" } else { "" })
                .into_any_element()
        };

        let line_stats = self.file_diff_stats.get(&path).copied();
        let stats_element = line_stats.map(|(additions, deletions)| {
            div()
                .flex()
                .gap_1()
                .text_xs()
                .font_family("Monaco")
                .child(
                    div()
                        .text_color(if selected {
                            colors.panel_background
                        } else {
                            colors.success
                        })
                        .child(format!("+{additions}")),
                )
                .child(
                    div()
                        .text_color(if selected {
                            colors.panel_background
                        } else {
                            colors.danger
                        })
                        .child(format!("-{deletions}")),
                )
                .into_any_element()
        });
        let mut row = div()
            .id(id)
            .h(px(22.0))
            .px_2()
            .flex()
            .items_center()
            .gap_2()
            .border_b_1()
            .border_color(colors.list_row_border)
            .bg(if selected {
                colors.accent
            } else {
                colors.panel_background
            })
            .text_color(if selected {
                colors.panel_background
            } else {
                colors.text_primary
            })
            .cursor_pointer();
        row.interactivity()
            .on_click(cx.listener(move |app, event: &ClickEvent, _, cx| {
                app.select_status_path(
                    path.clone(),
                    event.modifiers().secondary(),
                    event.modifiers().shift,
                    staged,
                    cx,
                );
            }));
        row.on_mouse_down(
            MouseButton::Right,
            cx.listener(move |app, _, _, cx| {
                app.show_status_context_menu(context_path.clone(), cx);
            }),
        )
        .child(checkbox)
        .child(status_badge_square(&badge_char, badge_bg, colors))
        .child(
            div()
                .flex_1()
                .overflow_hidden()
                .whitespace_nowrap()
                .text_ellipsis()
                .text_xs()
                .text_color(if selected {
                    colors.panel_background
                } else {
                    colors.text_secondary
                })
                .child(display_path.to_owned()),
        )
        .children(stats_element)
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

fn entry_is_staged(entry: &StatusEntry) -> bool {
    match entry {
        StatusEntry::Ordinary { status, .. } | StatusEntry::Renamed { status, .. } => {
            status.0[0] != b'.'
        }
        _ => false,
    }
}

fn file_list_mode_tab(
    label: &'static str,
    active: bool,
    colors: &ThemeColors,
    cx: &mut gpui::Context<GitronimoApp>,
    on_click: impl Fn(&mut GitronimoApp, &mut gpui::Context<GitronimoApp>) + 'static,
) -> gpui::AnyElement {
    let mut tab = div()
        .id(label)
        .px_2()
        .py_0p5()
        .rounded(px(3.0))
        .text_xs()
        .font_weight(gpui::FontWeight::MEDIUM)
        .bg(if active {
            colors.panel_background
        } else {
            colors.raised_background
        })
        .text_color(if active {
            colors.text_primary
        } else {
            colors.text_muted
        })
        .cursor_pointer();
    tab.interactivity()
        .on_click(cx.listener(move |app, _, _, cx| on_click(app, cx)));
    tab.child(label).into_any_element()
}

fn menu_item(
    label: &'static str,
    colors: &ThemeColors,
    cx: &mut gpui::Context<GitronimoApp>,
    on_click: impl Fn(&mut GitronimoApp, &ClickEvent, &mut gpui::Context<GitronimoApp>) + 'static,
) -> gpui::AnyElement {
    let mut item = div()
        .id(label)
        .h(px(24.0))
        .px_2()
        .flex()
        .items_center()
        .text_sm()
        .rounded(px(3.0))
        .cursor_pointer()
        .hover(|style| style.bg(colors.selection));
    item.interactivity()
        .on_click(cx.listener(move |app, event, _, cx| on_click(app, event, cx)));
    item.child(label).into_any_element()
}

fn menu_separator(colors: &ThemeColors) -> gpui::AnyElement {
    div().h(px(1.0)).my_1().bg(colors.border).into_any_element()
}
