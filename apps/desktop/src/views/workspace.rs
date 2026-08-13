//! The root window layout: toolbar, sidebar, content, activity bar.

use gpui::{
    AnyElement, Focusable, MouseButton, MouseDownEvent, Render, SharedString, Window, div,
    prelude::*, px, relative,
};
use ui_kit::Theme;

use crate::app_state::{
    AppConfirmDialog, ChoicePromptKind, GitronimoApp, OverlayFocus, PaletteCommand, PushOption,
    ShellState, ShortcutReferenceState, SubmodulePushMode, TextPromptKind, window_title,
};

use crate::views::components::{
    activity_color, activity_kind_color, activity_label, error_view, file_action_button,
    loading_view, primary_action_button, sidebar_resize_handle,
};
use crate::views::icons::{IconKind, icon};

impl Render for GitronimoApp {
    #[allow(clippy::too_many_lines)]
    fn render(&mut self, window: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        window.set_window_title(&window_title(&self.state, self.has_commit_draft()));
        if let Some(focus) = self.pending_overlay_focus.take() {
            let handle = match focus {
                OverlayFocus::CommandPalette => {
                    self.command_palette_input.read(cx).focus_handle(cx)
                }
                OverlayFocus::TextPrompt => self.text_prompt_input.read(cx).focus_handle(cx),
                OverlayFocus::ChoicePrompt => self.choice_prompt_input.read(cx).focus_handle(cx),
            };
            window.focus(&handle);
        }
        let colors = Theme::for_appearance(self.appearance).colors;
        let sidebar_width = self.sidebar_width;
        let content = match &self.state {
            ShellState::Welcome => self.welcome_view(&colors, cx).into_any_element(),
            ShellState::Loading(path) => loading_view(path, &colors).into_any_element(),
            ShellState::Repository(repository) => self
                .repository_view(repository, &colors, cx)
                .into_any_element(),
            ShellState::Error(message) => error_view(message, &colors).into_any_element(),
        };

        div()
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .bg(colors.window_background)
            .text_color(colors.text_primary)
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::refresh))
            .on_action(cx.listener(Self::focus_composer))
            .on_action(cx.listener(Self::show_command_palette))
            .on_action(cx.listener(Self::toggle_shortcut_reference))
            .on_action(cx.listener(Self::history_previous))
            .on_action(cx.listener(Self::history_next))
            .on_action(cx.listener(Self::navigate_back))
            .on_action(cx.listener(Self::navigate_forward))
            .on_action(cx.listener(Self::toggle_appearance))
            .on_action(cx.listener(Self::widen_sidebar))
            .on_action(cx.listener(Self::select_all_status_files))
            .on_action(cx.listener(Self::save_stash_shortcut))
            .on_drop(cx.listener(Self::dropped_paths))
            .child(self.workspace_toolbar(&colors, cx))
            .child(
                div()
                    .flex_1()
                    .flex()
                    .h_full()
                    .min_h(px(0.0))
                    .child(self.sidebar_view(sidebar_width, &colors, cx))
                    .child(sidebar_resize_handle(sidebar_width, &colors, cx))
                    .child(
                        div()
                            .flex_1()
                            .h_full()
                            .overflow_hidden()
                            .child(content)
                            .children(self.shortcut_reference_view(&colors, cx)),
                    ),
            )
            .child(
                div()
                    .min_h(px(26.0))
                    .px_3()
                    .flex()
                    .items_center()
                    .gap_2()
                    .bg(colors.panel_background)
                    .border_t_1()
                    .border_color(colors.border)
                    .text_xs()
                    .text_color(activity_color(&self.activity, &colors))
                    .child(self.activity_log_button(&colors, cx))
                    .children(self.network_activity_progress(&colors, cx))
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.0))
                            .overflow_hidden()
                            .child(activity_label(&self.activity)),
                    ),
            )
            // Overlays after chrome so they paint above in-flow content.
            .children(
                self.show_quick_open
                    .then(|| self.quick_open_overlay(&colors, cx).into_any_element()),
            )
            .children(
                self.show_command_palette
                    .then(|| self.command_palette_overlay(&colors, cx).into_any_element()),
            )
            .children(
                self.pending_text_prompt
                    .is_some()
                    .then(|| self.text_prompt_overlay(&colors, cx).into_any_element()),
            )
            .children(
                self.pending_choice_prompt
                    .is_some()
                    .then(|| self.choice_prompt_overlay(&colors, cx).into_any_element()),
            )
            .children(self.welcome_plus_menu_open.then(|| {
                self.welcome_plus_menu_overlay(&colors, cx)
                    .into_any_element()
            }))
            .children(self.ref_context.is_some().then(|| {
                self.ref_context_menu_overlay(&colors, cx)
                    .into_any_element()
            }))
            .children(self.commit_context.is_some().then(|| {
                self.commit_context_menu_overlay(&colors, cx)
                    .into_any_element()
            }))
            .children(
                self.pull_dialog
                    .is_some()
                    .then(|| self.pull_dialog_overlay(&colors, cx).into_any_element()),
            )
            .children(
                self.push_dialog
                    .is_some()
                    .then(|| self.push_dialog_overlay(&colors, cx).into_any_element()),
            )
            .children(self.stash_apply_dialog.is_some().then(|| {
                self.stash_apply_dialog_overlay(&colors, cx)
                    .into_any_element()
            }))
            .children(self.pending_branch_delete.is_some().then(|| {
                self.branch_delete_confirm_overlay(&colors, cx)
                    .into_any_element()
            }))
            .children(self.confirm_dialog.is_some().then(|| {
                self.app_confirm_dialog_overlay(&colors, cx)
                    .into_any_element()
            }))
            .children(
                self.show_activity_log
                    .then(|| self.activity_log_overlay(&colors, cx).into_any_element()),
            )
    }
}

impl GitronimoApp {
    pub(crate) fn shortcut_reference_view(
        &self,
        colors: &ui_kit::ThemeColors,
        cx: &mut gpui::Context<Self>,
    ) -> Option<AnyElement> {
        (self.shortcut_reference_state == ShortcutReferenceState::Visible).then(|| {
            div()
                .mt_4()
                .p_3()
                .flex()
                .flex_col()
                .gap_1()
                .bg(colors.raised_background)
                .border_1()
                .border_color(colors.border)
                .child("Keyboard shortcuts")
                .child("Command-O  Open repository")
                .child("Command-R  Refresh working copy")
                .child("Command-A  Select all changed files (Working Copy)")
                .child("Command-Shift-C  Edit commit subject")
                .child("Command-Shift-S  Save stash")
                .child("Command-Shift-P  Command palette")
                .child("Command-/  Show or hide this reference")
                .child("Command-[ / Command-]  Back / Forward")
                .child("Up / Down  Move through loaded history")
                .child("Command-Q  Quit Gitronimo")
                .child(file_action_button(
                    "Hide shortcut reference",
                    colors,
                    cx,
                    |app, cx| {
                        app.shortcut_reference_state = ShortcutReferenceState::Hidden;
                        cx.notify();
                    },
                ))
                .into_any_element()
        })
    }

    pub(crate) fn quick_open_overlay(
        &self,
        colors: &ui_kit::ThemeColors,
        cx: &mut gpui::Context<Self>,
    ) -> AnyElement {
        let recents = self.recents.clone();
        div()
            .absolute()
            .top(px(56.0))
            .left_0()
            .right_0()
            .bottom_0()
            .bg(colors.overlay_scrim)
            .flex()
            .items_start()
            .justify_center()
            .pt_8()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|app, _: &MouseDownEvent, _, cx| {
                    app.show_quick_open = false;
                    cx.notify();
                }),
            )
            .child(
                div()
                    .w(px(420.0))
                    .max_h(px(360.0))
                    .overflow_hidden()
                    .flex()
                    .flex_col()
                    .bg(colors.panel_background)
                    .border_1()
                    .border_color(colors.border)
                    .rounded(px(8.0))
                    .shadow_lg()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|_, _: &MouseDownEvent, _, cx| {
                            cx.stop_propagation();
                        }),
                    )
                    .child(
                        div()
                            .px_3()
                            .py_2()
                            .border_b_1()
                            .border_color(colors.border)
                            .text_sm()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child("Quick Open"),
                    )
                    .child(div().px_3().py_2().child(
                        crate::views::single_line_input::single_line_input_shell(
                            self.welcome_search_input.clone(),
                            colors,
                            false,
                        ),
                    ))
                    .children(recents.into_iter().enumerate().map(|(index, path)| {
                        let display = path.display().to_string();
                        div()
                            .id(index)
                            .px_3()
                            .py_2()
                            .text_sm()
                            .cursor_pointer()
                            .hover(|row| row.bg(colors.selection))
                            .on_click(cx.listener(move |app, _, window, cx| {
                                app.show_quick_open = false;
                                app.open_recent(path.clone(), window, cx);
                            }))
                            .child(display)
                            .into_any_element()
                    })),
            )
            .into_any_element()
    }

    #[allow(clippy::too_many_lines)]
    pub(crate) fn command_palette_overlay(
        &self,
        colors: &ui_kit::ThemeColors,
        cx: &mut gpui::Context<Self>,
    ) -> AnyElement {
        let commands = PaletteCommand::filtered(&self.command_palette_query);
        let selected = self
            .command_palette_selected
            .min(commands.len().saturating_sub(1));
        div()
            .absolute()
            .top(px(56.0))
            .left_0()
            .right_0()
            .bottom_0()
            .bg(colors.overlay_scrim)
            .flex()
            .items_start()
            .justify_center()
            .pt_8()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|app, _: &MouseDownEvent, _, cx| {
                    app.close_command_palette(cx);
                }),
            )
            .child(
                div()
                    .w(px(480.0))
                    .max_h(px(420.0))
                    .overflow_hidden()
                    .flex()
                    .flex_col()
                    .bg(colors.panel_background)
                    .border_1()
                    .border_color(colors.border)
                    .rounded(px(8.0))
                    .shadow_lg()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|_, _: &MouseDownEvent, _, cx| {
                            cx.stop_propagation();
                        }),
                    )
                    .child(
                        div()
                            .px_3()
                            .py_2()
                            .border_b_1()
                            .border_color(colors.border)
                            .text_sm()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child("Commands"),
                    )
                    .child(div().px_3().py_2().child(
                        crate::views::single_line_input::single_line_input_shell(
                            self.command_palette_input.clone(),
                            colors,
                            false,
                        ),
                    ))
                    .child(
                        div()
                            .id("command-palette-scroll")
                            .flex_1()
                            .min_h(px(0.0))
                            .max_h(px(320.0))
                            .overflow_y_scroll()
                            .children(if commands.is_empty() {
                                vec![
                                    div()
                                        .px_3()
                                        .py_2()
                                        .text_sm()
                                        .text_color(colors.text_muted)
                                        .child("No matching commands.")
                                        .into_any_element(),
                                ]
                            } else {
                                commands
                                    .into_iter()
                                    .enumerate()
                                    .map(|(row, (_, label, command))| {
                                        let selected_row = row == selected;
                                        div()
                                            .id(("command-palette-row", row))
                                            .px_3()
                                            .py_2()
                                            .text_sm()
                                            .cursor_pointer()
                                            .bg(if selected_row {
                                                colors.selection
                                            } else {
                                                colors.panel_background
                                            })
                                            .hover(|row| row.bg(colors.selection))
                                            .on_click(cx.listener(move |app, _, _, cx| {
                                                app.run_palette_command(command, cx);
                                            }))
                                            .child(label)
                                            .into_any_element()
                                    })
                                    .collect()
                            }),
                    ),
            )
            .into_any_element()
    }

    #[allow(clippy::too_many_lines)]
    pub(crate) fn text_prompt_overlay(
        &self,
        colors: &ui_kit::ThemeColors,
        cx: &mut gpui::Context<Self>,
    ) -> AnyElement {
        let Some(kind) = self.pending_text_prompt.clone() else {
            return div().into_any_element();
        };
        let (title, confirm_label) = match &kind {
            TextPromptKind::BranchRename { current } => {
                (format!("Rename branch '{current}'"), "Rename")
            }
            TextPromptKind::CreateBranch { .. } => ("New branch".into(), "Create"),
            TextPromptKind::CreateTag { start } => (format!("New tag from {start}"), "Create tag"),
            TextPromptKind::CreateStash { .. } => ("Save stash".into(), "Save"),
            TextPromptKind::StashBranch { reference } => {
                (format!("Branch from {reference}"), "Create branch")
            }
            TextPromptKind::FileHistoryPath => ("File history for path".into(), "Show history"),
            TextPromptKind::BlamePath => ("Blame path".into(), "Show blame"),
            TextPromptKind::CompareFrom => ("Compare from ref".into(), "Next"),
            TextPromptKind::CompareTo { left } => {
                (format!("Compare to ref (from {left})"), "Compare")
            }
            TextPromptKind::DropCommit => ("Drop commit".into(), "Drop"),
            TextPromptKind::BrowseTree => ("Browse tree at commit".into(), "Browse"),
            TextPromptKind::HistorySearch => ("Search loaded history".into(), "Search"),
            TextPromptKind::HistoryReference => ("Branch or tag history".into(), "Show"),
            TextPromptKind::RebaseOnto => ("Rebase onto".into(), "Rebase"),
            TextPromptKind::MergeRevision => ("Merge revision into current branch".into(), "Merge"),
            TextPromptKind::AutosquashTarget { squash } => (
                if *squash {
                    "Squash into commit".into()
                } else {
                    "Fixup into commit".into()
                },
                if *squash { "Next" } else { "Fixup" },
            ),
            TextPromptKind::AutosquashMessage { .. } => ("Squash message".into(), "Squash"),
            TextPromptKind::RewordSubject => ("New commit subject".into(), "Next"),
            TextPromptKind::RewordBody { .. } => ("New commit body (optional)".into(), "Reword"),
            TextPromptKind::MergeToolPath => {
                ("Conflicted path (leave empty for all)".into(), "Open tool")
            }
            TextPromptKind::CreateBookmarkFolder => ("New group".into(), "Create"),
            TextPromptKind::RenameBookmarkFolder { .. } => ("Rename group".into(), "Rename"),
        };
        div()
            .absolute()
            .top(px(56.0))
            .left_0()
            .right_0()
            .bottom_0()
            .bg(colors.overlay_scrim)
            .flex()
            .items_start()
            .justify_center()
            .pt_8()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|app, _: &MouseDownEvent, _, cx| {
                    app.cancel_text_prompt(cx);
                }),
            )
            .child(
                div()
                    .w(px(420.0))
                    .flex()
                    .flex_col()
                    .flex_shrink_0()
                    .bg(colors.panel_background)
                    .border_1()
                    .border_color(colors.border)
                    .rounded(px(8.0))
                    .shadow_lg()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|_, _: &MouseDownEvent, _, cx| {
                            cx.stop_propagation();
                        }),
                    )
                    .child(
                        div()
                            .px_3()
                            .py_2()
                            .border_b_1()
                            .border_color(colors.border)
                            .text_sm()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child(title),
                    )
                    .child(div().px_3().py_2().child(
                        crate::views::single_line_input::single_line_input_shell(
                            self.text_prompt_input.clone(),
                            colors,
                            false,
                        ),
                    ))
                    .children(matches!(&kind, TextPromptKind::CreateStash { .. }).then(|| {
                        let checked = matches!(
                            &kind,
                            TextPromptKind::CreateStash {
                                include_untracked: true,
                                ..
                            }
                        );
                        let path_note = match &kind {
                            TextPromptKind::CreateStash { paths, .. } if !paths.is_empty() => {
                                format!("Stashing {} selected path(s).", paths.len())
                            }
                            _ => String::new(),
                        };
                        div()
                            .px_3()
                            .pb_2()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child({
                                let mut row = div()
                                    .id("stash-include-untracked")
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .cursor_pointer()
                                    .text_sm();
                                row.interactivity().on_click(cx.listener(
                                    |app, _: &gpui::ClickEvent, _, cx| {
                                        app.toggle_create_stash_include_untracked(cx);
                                    },
                                ));
                                row.child(
                                    div()
                                        .w(px(14.0))
                                        .h(px(14.0))
                                        .rounded(px(3.0))
                                        .border_1()
                                        .border_color(if checked {
                                            colors.accent
                                        } else {
                                            colors.border
                                        })
                                        .bg(if checked {
                                            colors.accent
                                        } else {
                                            colors.panel_background
                                        })
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .text_color(colors.panel_background)
                                        .text_xs()
                                        .child(if checked { "✓" } else { "" }),
                                )
                                .child("Include untracked files")
                            })
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(colors.text_muted)
                                    .child(
                                        "Including untracked files can remove ignored folders depending on ignore rules (Git behavior).",
                                    ),
                            )
                            .children((!path_note.is_empty()).then(|| {
                                div()
                                    .text_xs()
                                    .text_color(colors.text_muted)
                                    .child(path_note)
                            }))
                            .into_any_element()
                    }))
                    .child(
                        div()
                            .px_3()
                            .py_2()
                            .flex()
                            .gap_2()
                            .child(primary_action_button(
                                confirm_label,
                                colors,
                                cx,
                                |app, cx| {
                                    app.confirm_text_prompt(cx);
                                },
                            ))
                            .child(file_action_button("Cancel", colors, cx, |app, cx| {
                                app.cancel_text_prompt(cx);
                            })),
                    ),
            )
            .into_any_element()
    }

    pub(crate) fn stash_apply_dialog_overlay(
        &self,
        colors: &ui_kit::ThemeColors,
        cx: &mut gpui::Context<Self>,
    ) -> AnyElement {
        let Some(dialog) = self.stash_apply_dialog.clone() else {
            return div().into_any_element();
        };
        div()
            .absolute()
            .top(px(56.0))
            .left_0()
            .right_0()
            .bottom_0()
            .bg(colors.overlay_scrim)
            .flex()
            .items_start()
            .justify_center()
            .pt_8()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|app, _: &MouseDownEvent, _, cx| {
                    app.close_stash_apply_dialog(cx);
                }),
            )
            .child(
                div()
                    .w(px(420.0))
                    .flex()
                    .flex_col()
                    .bg(colors.panel_background)
                    .border_1()
                    .border_color(colors.border)
                    .rounded(px(8.0))
                    .shadow_lg()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|_, _: &MouseDownEvent, _, cx| {
                            cx.stop_propagation();
                        }),
                    )
                    .child(
                        div()
                            .px_3()
                            .py_2()
                            .border_b_1()
                            .border_color(colors.border)
                            .text_sm()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child(format!("Apply {}", dialog.reference)),
                    )
                    .child(
                        div()
                            .px_3()
                            .py_2()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(stash_option_row(
                                "stash-delete-after",
                                "Delete stash after applying changes",
                                dialog.delete_after,
                                colors,
                                cx,
                                GitronimoApp::toggle_stash_apply_delete_after,
                            ))
                            .child(stash_option_row(
                                "stash-restore-index",
                                "Restore staging area status",
                                dialog.restore_index,
                                colors,
                                cx,
                                GitronimoApp::toggle_stash_apply_restore_index,
                            )),
                    )
                    .child(
                        div()
                            .px_3()
                            .py_2()
                            .border_t_1()
                            .border_color(colors.border)
                            .flex()
                            .justify_end()
                            .gap_2()
                            .child(file_action_button("Cancel", colors, cx, |app, cx| {
                                app.close_stash_apply_dialog(cx);
                            }))
                            .child(primary_action_button("Apply", colors, cx, |app, cx| {
                                app.confirm_stash_apply_dialog(cx);
                            })),
                    ),
            )
            .into_any_element()
    }

    #[allow(clippy::too_many_lines)]
    pub(crate) fn choice_prompt_overlay(
        &self,
        colors: &ui_kit::ThemeColors,
        cx: &mut gpui::Context<Self>,
    ) -> AnyElement {
        let Some(kind) = self.pending_choice_prompt.clone() else {
            return div().into_any_element();
        };
        let title = kind.title();
        let is_confirm = matches!(kind, ChoicePromptKind::ConfirmMergePullRequest { .. });
        let options = kind.filtered_options(&self.choice_prompt_query);
        let selected = self
            .choice_prompt_selected
            .min(options.len().saturating_sub(1));
        div()
            .absolute()
            .top(px(56.0))
            .left_0()
            .right_0()
            .bottom_0()
            .bg(colors.overlay_scrim)
            .flex()
            .items_start()
            .justify_center()
            .pt_8()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|app, _: &MouseDownEvent, _, cx| {
                    app.cancel_choice_prompt(cx);
                }),
            )
            .child(
                div()
                    .w(px(420.0))
                    .max_h(px(360.0))
                    .overflow_hidden()
                    .flex()
                    .flex_col()
                    .bg(colors.panel_background)
                    .border_1()
                    .border_color(colors.border)
                    .rounded(px(8.0))
                    .shadow_lg()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|_, _: &MouseDownEvent, _, cx| {
                            cx.stop_propagation();
                        }),
                    )
                    .child(
                        div()
                            .px_3()
                            .py_2()
                            .border_b_1()
                            .border_color(colors.border)
                            .text_sm()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child(title),
                    )
                    .child(
                        div()
                            .when(is_confirm, |el| el.h(px(0.0)).overflow_hidden())
                            .when(!is_confirm, |el| el.px_3().py_2())
                            .child(crate::views::single_line_input::single_line_input_shell(
                                self.choice_prompt_input.clone(),
                                colors,
                                false,
                            )),
                    )
                    .children(is_confirm.then(|| {
                        div()
                            .px_3()
                            .py_2()
                            .flex()
                            .gap_2()
                            .child(primary_action_button("Merge", colors, cx, |app, cx| {
                                app.confirm_choice_prompt(cx);
                            }))
                            .child(file_action_button("Cancel", colors, cx, |app, cx| {
                                app.cancel_choice_prompt(cx);
                            }))
                    }))
                    .children((!is_confirm).then(|| {
                        div().flex_1().overflow_hidden().children(
                            options.into_iter().enumerate().map(|(row, (_, label))| {
                                let selected_row = row == selected;
                                let prompt_kind = kind.clone();
                                div()
                                    .id(row)
                                    .px_3()
                                    .py_2()
                                    .text_sm()
                                    .cursor_pointer()
                                    .bg(if selected_row {
                                        colors.selection
                                    } else {
                                        colors.panel_background
                                    })
                                    .hover(|row| row.bg(colors.selection))
                                    .on_click(cx.listener(move |app, _, _, cx| {
                                        app.select_choice_option(&prompt_kind, label, cx);
                                    }))
                                    .child(label)
                                    .into_any_element()
                            }),
                        )
                    })),
            )
            .into_any_element()
    }

    #[allow(clippy::unused_self)]
    pub(crate) fn welcome_plus_menu_overlay(
        &self,
        colors: &ui_kit::ThemeColors,
        cx: &mut gpui::Context<Self>,
    ) -> AnyElement {
        // Anchor near the welcome sidebar footer `+` (toolbar 56 + activity 26 + footer ~40).
        let menu_bottom = px(66.0);
        let menu_left = px(12.0);
        div()
            .absolute()
            .inset_0()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|app, _: &MouseDownEvent, _, cx| {
                    app.close_welcome_plus_menu(cx);
                }),
            )
            .child(
                div()
                    .id("welcome-plus-menu")
                    .absolute()
                    .bottom(menu_bottom)
                    .left(menu_left)
                    .min_w(px(200.0))
                    .py_1()
                    .bg(colors.panel_background)
                    .border_1()
                    .border_color(colors.border)
                    .rounded(px(8.0))
                    .shadow_lg()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|_, _: &MouseDownEvent, _, cx| {
                            cx.stop_propagation();
                        }),
                    )
                    .child(welcome_plus_menu_item(
                        "New Group…",
                        colors,
                        cx,
                        |app, cx| {
                            app.new_bookmark_group_from_menu(cx);
                        },
                    ))
                    .child(welcome_plus_menu_item(
                        "Add Repository…",
                        colors,
                        cx,
                        |app, cx| {
                            app.add_repository_from_picker(cx);
                        },
                    ))
                    .child(
                        div()
                            .h(px(1.0))
                            .my_1()
                            .mx_2()
                            .bg(colors.border)
                            .into_any_element(),
                    )
                    .child(welcome_plus_menu_item(
                        "Create Repository…",
                        colors,
                        cx,
                        |app, cx| {
                            app.close_welcome_plus_menu(cx);
                            app.prompt_create_repository(cx);
                        },
                    ))
                    .child(welcome_plus_menu_item(
                        "Clone Repository…",
                        colors,
                        cx,
                        |app, cx| {
                            app.close_welcome_plus_menu(cx);
                            app.prompt_clone_repository(cx);
                        },
                    )),
            )
            .into_any_element()
    }

    #[allow(clippy::too_many_lines)]
    pub(crate) fn pull_dialog_overlay(
        &self,
        colors: &ui_kit::ThemeColors,
        cx: &mut gpui::Context<Self>,
    ) -> AnyElement {
        let Some(dialog) = self.pull_dialog.clone() else {
            return div().into_any_element();
        };
        let selected_label = if dialog.remote_branch.is_empty() {
            "Configured upstream".into()
        } else {
            dialog.remote_branch.clone()
        };
        div()
            .absolute()
            .top(px(56.0))
            .left_0()
            .right_0()
            .bottom_0()
            .bg(colors.overlay_scrim)
            .flex()
            .items_start()
            .justify_center()
            .pt_8()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|app, _: &MouseDownEvent, _, cx| {
                    app.close_pull_dialog(cx);
                }),
            )
            .child(
                div()
                    .w(px(420.0))
                    .max_h(px(480.0))
                    .overflow_hidden()
                    .flex()
                    .flex_col()
                    .bg(colors.panel_background)
                    .border_1()
                    .border_color(colors.border)
                    .rounded(px(8.0))
                    .shadow_lg()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|_, _: &MouseDownEvent, _, cx| {
                            cx.stop_propagation();
                        }),
                    )
                    .child(
                        div()
                            .px_3()
                            .py_2()
                            .border_b_1()
                            .border_color(colors.border)
                            .text_sm()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child("Pull"),
                    )
                    .child(
                        div()
                            .px_3()
                            .py_2()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(colors.text_muted)
                                    .child("Remote Branch"),
                            )
                            .child({
                                let mut field = div()
                                    .id("pull-remote-branch")
                                    .h(px(30.0))
                                    .px_2()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .rounded(px(4.0))
                                    .border_1()
                                    .border_color(colors.border)
                                    .bg(colors.raised_background)
                                    .cursor_pointer()
                                    .hover(|style| style.bg(colors.selection));
                                field.interactivity().on_click(cx.listener(
                                    |app, _: &gpui::ClickEvent, _, cx| {
                                        app.toggle_pull_dialog_branch_menu(cx);
                                    },
                                ));
                                field
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w(px(0.0))
                                            .overflow_hidden()
                                            .whitespace_nowrap()
                                            .text_sm()
                                            .child(selected_label),
                                    )
                                    .child(div().text_xs().text_color(colors.text_muted).child("▾"))
                            })
                            .children(dialog.branch_menu_open.then(|| {
                                div()
                                    .max_h(px(160.0))
                                    .overflow_hidden()
                                    .rounded(px(4.0))
                                    .border_1()
                                    .border_color(colors.border)
                                    .bg(colors.raised_background)
                                    .children(dialog.remote_branches.iter().enumerate().map(
                                        |(index, name)| {
                                            let selected = name == &dialog.remote_branch;
                                            let choice = name.clone();
                                            let mut row = div()
                                                .id(("pull-branch", index))
                                                .px_2()
                                                .py_1()
                                                .text_sm()
                                                .cursor_pointer()
                                                .bg(if selected {
                                                    colors.accent
                                                } else {
                                                    colors.raised_background
                                                })
                                                .text_color(if selected {
                                                    colors.panel_background
                                                } else {
                                                    colors.text_primary
                                                })
                                                .hover(|style| style.bg(colors.selection));
                                            row.interactivity().on_click(cx.listener(
                                                move |app, _: &gpui::ClickEvent, _, cx| {
                                                    app.select_pull_dialog_remote_branch(
                                                        choice.clone(),
                                                        cx,
                                                    );
                                                },
                                            ));
                                            row.child(name.clone())
                                        },
                                    ))
                                    .into_any_element()
                            })),
                    )
                    .child(
                        div()
                            .px_3()
                            .py_2()
                            .border_t_1()
                            .border_color(colors.border)
                            .child({
                                let checked = dialog.use_rebase;
                                let mut row = div()
                                    .id("pull-use-rebase")
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .cursor_pointer()
                                    .text_sm();
                                row.interactivity().on_click(cx.listener(
                                    |app, _: &gpui::ClickEvent, _, cx| {
                                        app.toggle_pull_dialog_rebase(cx);
                                    },
                                ));
                                row.child(
                                    div()
                                        .w(px(14.0))
                                        .h(px(14.0))
                                        .rounded(px(3.0))
                                        .border_1()
                                        .border_color(if checked {
                                            colors.accent
                                        } else {
                                            colors.border
                                        })
                                        .bg(if checked {
                                            colors.accent
                                        } else {
                                            colors.panel_background
                                        })
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .text_color(colors.panel_background)
                                        .text_xs()
                                        .child(if checked { "✓" } else { "" }),
                                )
                                .child("Use Rebase Instead of Merge")
                            }),
                    )
                    .child(
                        div()
                            .px_3()
                            .py_2()
                            .border_t_1()
                            .border_color(colors.border)
                            .flex()
                            .justify_end()
                            .gap_2()
                            .child(file_action_button("Cancel", colors, cx, |app, cx| {
                                app.close_pull_dialog(cx);
                            }))
                            .child(primary_action_button("Pull", colors, cx, |app, cx| {
                                app.confirm_pull_dialog(cx);
                            })),
                    ),
            )
            .into_any_element()
    }

    #[allow(clippy::too_many_lines)]
    pub(crate) fn push_dialog_overlay(
        &self,
        colors: &ui_kit::ThemeColors,
        cx: &mut gpui::Context<Self>,
    ) -> AnyElement {
        let Some(dialog) = self.push_dialog.clone() else {
            return div().into_any_element();
        };
        let destination_label = if dialog.destination.is_empty() {
            "Configured upstream".into()
        } else {
            dialog.destination.clone()
        };
        let description = format!(
            "Pushes new commits from your local HEAD branch \"{}\" to the chosen remote branch.",
            dialog.head_branch
        );
        div()
            .absolute()
            .top(px(56.0))
            .left_0()
            .right_0()
            .bottom_0()
            .bg(colors.overlay_scrim)
            .flex()
            .items_start()
            .justify_center()
            .pt_8()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|app, _: &MouseDownEvent, _, cx| {
                    app.close_push_dialog(cx);
                }),
            )
            .child(
                div()
                    .w(px(460.0))
                    .max_h(px(600.0))
                    .overflow_hidden()
                    .flex()
                    .flex_col()
                    .bg(colors.panel_background)
                    .border_1()
                    .border_color(colors.border)
                    .rounded(px(8.0))
                    .shadow_lg()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|_, _: &MouseDownEvent, _, cx| {
                            cx.stop_propagation();
                        }),
                    )
                    .child(
                        div()
                            .px_4()
                            .py_3()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .child("Push HEAD"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(colors.text_muted)
                                    .child(description),
                            ),
                    )
                    .child(
                        div()
                            .px_4()
                            .pb_3()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(colors.text_muted)
                                    .child("Destination"),
                            )
                            .child({
                                let mut field = div()
                                    .id("push-destination")
                                    .h(px(30.0))
                                    .px_2()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .rounded(px(4.0))
                                    .border_1()
                                    .border_color(colors.border)
                                    .bg(colors.raised_background)
                                    .cursor_pointer()
                                    .hover(|style| style.bg(colors.selection));
                                field.interactivity().on_click(cx.listener(
                                    |app, _: &gpui::ClickEvent, _, cx| {
                                        app.toggle_push_dialog_destination_menu(cx);
                                    },
                                ));
                                field
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w(px(0.0))
                                            .overflow_hidden()
                                            .whitespace_nowrap()
                                            .text_sm()
                                            .child(destination_label),
                                    )
                                    .child(div().text_xs().text_color(colors.text_muted).child("▾"))
                            })
                            .children(dialog.destination_menu_open.then(|| {
                                div()
                                    .max_h(px(160.0))
                                    .overflow_hidden()
                                    .rounded(px(4.0))
                                    .border_1()
                                    .border_color(colors.border)
                                    .bg(colors.raised_background)
                                    .children(dialog.destinations.iter().enumerate().map(
                                        |(index, name)| {
                                            let selected = name == &dialog.destination;
                                            let choice = name.clone();
                                            let mut row = div()
                                                .id(("push-destination-choice", index))
                                                .px_2()
                                                .py_1()
                                                .text_sm()
                                                .cursor_pointer()
                                                .bg(if selected {
                                                    colors.accent
                                                } else {
                                                    colors.raised_background
                                                })
                                                .text_color(if selected {
                                                    colors.panel_background
                                                } else {
                                                    colors.text_primary
                                                })
                                                .hover(|style| style.bg(colors.selection));
                                            row.interactivity().on_click(cx.listener(
                                                move |app, _: &gpui::ClickEvent, _, cx| {
                                                    app.select_push_dialog_destination(
                                                        choice.clone(),
                                                        cx,
                                                    );
                                                },
                                            ));
                                            row.child(name.clone())
                                        },
                                    ))
                                    .into_any_element()
                            })),
                    )
                    .child(
                        div()
                            .px_4()
                            .py_3()
                            .border_t_1()
                            .border_color(colors.border)
                            .bg(colors.raised_background)
                            .flex()
                            .flex_col()
                            .gap_3()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(colors.text_muted)
                                    .child("Options"),
                            )
                            .child(push_option_row(
                                PushOption::AllTags,
                                dialog.is_enabled(PushOption::AllTags),
                                colors,
                                cx,
                            ))
                            .child(push_option_row(
                                PushOption::Force,
                                dialog.is_enabled(PushOption::Force),
                                colors,
                                cx,
                            ))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_2()
                                    .child(push_option_row(
                                        PushOption::RecurseSubmodules,
                                        dialog.is_enabled(PushOption::RecurseSubmodules),
                                        colors,
                                        cx,
                                    ))
                                    .child(
                                        div()
                                            .pl(px(22.0))
                                            .flex()
                                            .flex_col()
                                            .gap_1()
                                            .child({
                                                let enabled =
                                                    dialog.is_enabled(PushOption::RecurseSubmodules);
                                                let mut field = div()
                                                    .id("push-submodule-mode")
                                                    .h(px(26.0))
                                                    .px_2()
                                                    .flex()
                                                    .items_center()
                                                    .justify_between()
                                                    .rounded(px(4.0))
                                                    .border_1()
                                                    .border_color(colors.border)
                                                    .bg(colors.panel_background)
                                                    .text_color(if enabled {
                                                        colors.text_primary
                                                    } else {
                                                        colors.text_muted
                                                    });
                                                if enabled {
                                                    field = field
                                                        .cursor_pointer()
                                                        .hover(|style| style.bg(colors.selection));
                                                }
                                                field.interactivity().on_click(cx.listener(
                                                    |app, _: &gpui::ClickEvent, _, cx| {
                                                        app.toggle_push_dialog_submodule_menu(cx);
                                                    },
                                                ));
                                                field
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .child(dialog.submodule_mode.label()),
                                                    )
                                                    .child(div().text_xs().child("▾"))
                                            })
                                            .children(dialog.submodule_menu_open.then(|| {
                                                div()
                                                    .rounded(px(4.0))
                                                    .border_1()
                                                    .border_color(colors.border)
                                                    .bg(colors.panel_background)
                                                    .children(
                                                        SubmodulePushMode::choices().into_iter().enumerate().map(
                                                            |(index, mode)| {
                                                                let selected =
                                                                    mode == dialog.submodule_mode;
                                                                let mut row = div()
                                                                    .id(("push-submodule-mode-choice", index))
                                                                    .px_2()
                                                                    .py_1()
                                                                    .text_xs()
                                                                    .cursor_pointer()
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
                                                                    .hover(|style| {
                                                                        style.bg(colors.selection)
                                                                    });
                                                                row.interactivity().on_click(
                                                                    cx.listener(
                                                                        move |app, _: &gpui::ClickEvent, _, cx| {
                                                                            app.select_push_dialog_submodule_mode(mode, cx);
                                                                        },
                                                                    ),
                                                                );
                                                                row.child(mode.label())
                                                            },
                                                        ),
                                                    )
                                                    .into_any_element()
                                            })),
                                    ),
                            )
                            .child(push_option_row(
                                PushOption::SkipHooks,
                                dialog.is_enabled(PushOption::SkipHooks),
                                colors,
                                cx,
                            )),
                    )
                    .child(
                        div()
                            .px_4()
                            .py_3()
                            .border_t_1()
                            .border_color(colors.border)
                            .flex()
                            .justify_end()
                            .gap_2()
                            .child(file_action_button("Cancel", colors, cx, |app, cx| {
                                app.close_push_dialog(cx);
                            }))
                            .child(primary_action_button("Push HEAD", colors, cx, |app, cx| {
                                app.confirm_push_dialog(cx);
                            })),
                    ),
            )
            .into_any_element()
    }

    pub(crate) fn branch_delete_confirm_overlay(
        &self,
        colors: &ui_kit::ThemeColors,
        cx: &mut gpui::Context<Self>,
    ) -> AnyElement {
        let Some(branch) = self.pending_branch_delete.clone() else {
            return div().into_any_element();
        };
        Self::modal_confirm_overlay(
            colors,
            cx,
            "Delete Branch",
            format!("Do you really want to delete the branch \"{branch}\"?"),
            "Cancel",
            "Delete",
            GitronimoApp::cancel_branch_delete,
            |app, cx| app.confirm_branch_delete(false, cx),
        )
    }

    pub(crate) fn app_confirm_dialog_overlay(
        &self,
        colors: &ui_kit::ThemeColors,
        cx: &mut gpui::Context<Self>,
    ) -> AnyElement {
        let Some(dialog) = self.confirm_dialog.clone() else {
            return div().into_any_element();
        };
        Self::modal_confirm_overlay(
            colors,
            cx,
            dialog.title(),
            dialog.body(),
            AppConfirmDialog::cancel_label(),
            dialog.confirm_label(),
            GitronimoApp::cancel_confirm_dialog,
            GitronimoApp::confirm_confirm_dialog,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn modal_confirm_overlay(
        colors: &ui_kit::ThemeColors,
        cx: &mut gpui::Context<Self>,
        title: impl Into<SharedString>,
        body: impl Into<SharedString>,
        cancel_label: &'static str,
        confirm_label: &'static str,
        on_cancel: impl Fn(&mut GitronimoApp, &mut gpui::Context<GitronimoApp>) + 'static + Clone,
        on_confirm: impl Fn(&mut GitronimoApp, &mut gpui::Context<GitronimoApp>) + 'static + Clone,
    ) -> AnyElement {
        let on_cancel_scrim = on_cancel.clone();
        let on_cancel_btn = on_cancel;
        let on_confirm_btn = on_confirm;
        div()
            .absolute()
            .top(px(56.0))
            .left_0()
            .right_0()
            .bottom_0()
            .bg(colors.overlay_scrim)
            .flex()
            .items_start()
            .justify_center()
            .pt_8()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |app, _: &MouseDownEvent, _, cx| {
                    on_cancel_scrim(app, cx);
                }),
            )
            .child(
                div()
                    .w(px(420.0))
                    .flex()
                    .flex_col()
                    .bg(colors.panel_background)
                    .border_1()
                    .border_color(colors.border)
                    .rounded(px(8.0))
                    .shadow_lg()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|_, _: &MouseDownEvent, _, cx| {
                            cx.stop_propagation();
                        }),
                    )
                    .child(
                        div()
                            .px_3()
                            .py_2()
                            .border_b_1()
                            .border_color(colors.border)
                            .text_sm()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child(title.into()),
                    )
                    .child(
                        div()
                            .px_3()
                            .py_2()
                            .text_xs()
                            .text_color(colors.text_secondary)
                            .child(body.into()),
                    )
                    .child(
                        div()
                            .px_3()
                            .py_3()
                            .border_t_1()
                            .border_color(colors.border)
                            .flex()
                            .justify_end()
                            .gap_2()
                            .child(file_action_button(
                                cancel_label,
                                colors,
                                cx,
                                move |app, cx| {
                                    on_cancel_btn(app, cx);
                                },
                            ))
                            .child(primary_action_button(
                                confirm_label,
                                colors,
                                cx,
                                move |app, cx| {
                                    on_confirm_btn(app, cx);
                                },
                            )),
                    ),
            )
            .into_any_element()
    }

    fn activity_log_button(
        &self,
        colors: &ui_kit::ThemeColors,
        cx: &mut gpui::Context<Self>,
    ) -> AnyElement {
        let active = self.show_activity_log;
        div()
            .id("activity-log-button")
            .flex_shrink_0()
            .w(px(22.0))
            .h(px(20.0))
            .rounded(px(4.0))
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .bg(if active {
                colors.selection
            } else {
                colors.panel_background
            })
            .hover(|style| style.bg(colors.selection))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|app, _: &MouseDownEvent, _, cx| {
                    app.toggle_activity_log(cx);
                    cx.stop_propagation();
                }),
            )
            .child(icon(
                IconKind::History,
                12.0,
                if active {
                    colors.accent
                } else {
                    colors.text_muted
                },
            ))
            .into_any_element()
    }

    #[allow(clippy::too_many_lines)]
    pub(crate) fn activity_log_overlay(
        &self,
        colors: &ui_kit::ThemeColors,
        cx: &mut gpui::Context<Self>,
    ) -> AnyElement {
        let entries = self.activity_log.iter().cloned().collect::<Vec<_>>();
        let row_count = entries.len().max(1);
        let scroll_height = (f32::from(u16::try_from(row_count).unwrap_or(u16::MAX)) * 28.0 + 8.0)
            .clamp(80.0, 280.0);
        div()
            .absolute()
            .inset_0()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|app, _: &MouseDownEvent, _, cx| {
                    app.close_activity_log(cx);
                }),
            )
            .child(
                div()
                    .id("activity-log-popup")
                    .absolute()
                    .bottom(px(30.0))
                    .left(px(8.0))
                    .w(px(440.0))
                    .flex()
                    .flex_col()
                    .bg(colors.panel_background)
                    .border_1()
                    .border_color(colors.border)
                    .rounded(px(8.0))
                    .shadow_lg()
                    .overflow_hidden()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|_, _: &MouseDownEvent, _, cx| {
                            cx.stop_propagation();
                        }),
                    )
                    .child(
                        div()
                            .px_3()
                            .py_2()
                            .border_b_1()
                            .border_color(colors.border)
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .text_color(colors.text_primary)
                                    .child("Message history"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(colors.text_muted)
                                    .child(format!("{} recent", entries.len())),
                            ),
                    )
                    .child(
                        div()
                            .id("activity-log-scroll")
                            .h(px(scroll_height))
                            .overflow_y_scroll()
                            .py_1()
                            .children(if entries.is_empty() {
                                vec![
                                    div()
                                        .px_3()
                                        .py_2()
                                        .text_xs()
                                        .text_color(colors.text_muted)
                                        .child("No messages yet.")
                                        .into_any_element(),
                                ]
                            } else {
                                entries
                                    .into_iter()
                                    .enumerate()
                                    .map(|(index, entry)| {
                                        let color = activity_kind_color(entry.kind, colors);
                                        div()
                                            .id(("activity-log-row", index))
                                            .px_3()
                                            .py_1p5()
                                            .flex()
                                            .gap_2()
                                            .items_start()
                                            .hover(|style| style.bg(colors.selection))
                                            .child(
                                                div()
                                                    .mt(px(5.0))
                                                    .flex_shrink_0()
                                                    .w(px(6.0))
                                                    .h(px(6.0))
                                                    .rounded_full()
                                                    .bg(color),
                                            )
                                            .child(
                                                div()
                                                    .flex_shrink_0()
                                                    .w(px(40.0))
                                                    .text_xs()
                                                    .text_color(colors.text_muted)
                                                    .child(format_activity_age(entry.at)),
                                            )
                                            .child(
                                                div()
                                                    .flex_1()
                                                    .min_w(px(0.0))
                                                    .text_xs()
                                                    .text_color(color)
                                                    .child(entry.message),
                                            )
                                            .into_any_element()
                                    })
                                    .collect()
                            }),
                    ),
            )
            .into_any_element()
    }

    fn network_activity_progress(
        &self,
        colors: &ui_kit::ThemeColors,
        cx: &mut gpui::Context<Self>,
    ) -> Option<AnyElement> {
        let operation = self.network_operation.as_ref()?;
        let label = operation.lock().ok()?.label.clone();
        let fill = self.network_progress.clamp(0.08, 0.92);
        Some(
            div()
                .flex()
                .items_center()
                .gap_2()
                .flex_shrink_0()
                .child(
                    div()
                        .w(px(120.0))
                        .h(px(6.0))
                        .rounded_full()
                        .bg(colors.raised_background)
                        .border_1()
                        .border_color(colors.border)
                        .overflow_hidden()
                        .child(
                            div()
                                .h_full()
                                .w(relative(fill))
                                .bg(colors.accent)
                                .rounded_full(),
                        ),
                )
                .child(
                    div()
                        .max_w(px(220.0))
                        .overflow_hidden()
                        .text_color(colors.text_secondary)
                        .child(label),
                )
                .child(file_action_button("Cancel", colors, cx, |app, cx| {
                    app.cancel_network_operation(cx);
                }))
                .into_any_element(),
        )
    }
}

fn format_activity_age(at: std::time::SystemTime) -> String {
    let Ok(elapsed) = std::time::SystemTime::now().duration_since(at) else {
        return String::new();
    };
    let secs = elapsed.as_secs();
    if secs < 60 {
        format!("{secs}s")
    } else if secs < 3600 {
        format!("{}m", secs / 60)
    } else if secs < 86_400 {
        format!("{}h", secs / 3600)
    } else {
        format!("{}d", secs / 86_400)
    }
}

fn push_option_row(
    option: PushOption,
    checked: bool,
    colors: &ui_kit::ThemeColors,
    cx: &mut gpui::Context<GitronimoApp>,
) -> AnyElement {
    let mut row = div()
        .id(option.element_id())
        .flex()
        .items_start()
        .gap_2()
        .cursor_pointer();
    row.interactivity()
        .on_click(cx.listener(move |app, _: &gpui::ClickEvent, _, cx| {
            app.toggle_push_dialog_option(option, cx);
        }));
    row.child(
        div()
            .mt(px(2.0))
            .w(px(14.0))
            .h(px(14.0))
            .flex_none()
            .rounded(px(3.0))
            .border_1()
            .border_color(if checked {
                colors.accent
            } else {
                colors.border
            })
            .bg(if checked {
                colors.accent
            } else {
                colors.panel_background
            })
            .flex()
            .items_center()
            .justify_center()
            .text_color(colors.panel_background)
            .text_xs()
            .child(if checked { "✓" } else { "" }),
    )
    .child(
        div()
            .flex()
            .flex_col()
            .gap_0p5()
            .child(div().text_sm().child(option.label()))
            .child(
                div()
                    .text_xs()
                    .text_color(colors.text_muted)
                    .child(option.caption()),
            ),
    )
    .into_any_element()
}

fn stash_option_row(
    id: &'static str,
    label: &'static str,
    checked: bool,
    colors: &ui_kit::ThemeColors,
    cx: &mut gpui::Context<GitronimoApp>,
    on_click: impl Fn(&mut GitronimoApp, &mut gpui::Context<GitronimoApp>) + 'static,
) -> AnyElement {
    let mut row = div()
        .id(id)
        .flex()
        .items_center()
        .gap_2()
        .cursor_pointer()
        .text_sm();
    row.interactivity()
        .on_click(cx.listener(move |app, _: &gpui::ClickEvent, _, cx| {
            on_click(app, cx);
        }));
    row.child(
        div()
            .w(px(14.0))
            .h(px(14.0))
            .rounded(px(3.0))
            .border_1()
            .border_color(if checked {
                colors.accent
            } else {
                colors.border
            })
            .bg(if checked {
                colors.accent
            } else {
                colors.panel_background
            })
            .flex()
            .items_center()
            .justify_center()
            .text_color(colors.panel_background)
            .text_xs()
            .child(if checked { "✓" } else { "" }),
    )
    .child(label)
    .into_any_element()
}

fn welcome_plus_menu_item(
    label: &'static str,
    colors: &ui_kit::ThemeColors,
    cx: &mut gpui::Context<GitronimoApp>,
    on_click: impl Fn(&mut GitronimoApp, &mut gpui::Context<GitronimoApp>) + 'static,
) -> AnyElement {
    div()
        .id(gpui::ElementId::Name(
            format!("welcome-plus-menu:{label}").into(),
        ))
        .px_3()
        .py_1p5()
        .mx_1()
        .rounded(px(4.0))
        .text_sm()
        .cursor_pointer()
        .hover(|style| style.bg(colors.selection))
        .on_click(cx.listener(move |app, _, _, cx| on_click(app, cx)))
        .child(label)
        .into_any_element()
}
