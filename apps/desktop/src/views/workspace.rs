//! The root window layout: toolbar, sidebar, content, activity bar.

use gpui::{
    AnyElement, Focusable, MouseButton, MouseDownEvent, Render, Window, div, prelude::*, px,
};
use ui_kit::Theme;

use crate::app_state::{
    ChoicePromptKind, GitronimoApp, OverlayFocus, PaletteCommand, ShellState,
    ShortcutReferenceState, TextPromptKind, window_title,
};

use super::components::{
    activity_color, activity_label, error_view, file_action_button, loading_view,
    sidebar_resize_handle,
};

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
            .on_drop(cx.listener(Self::dropped_paths))
            .child(self.workspace_toolbar(&colors, cx))
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
            .child(
                div()
                    .flex_1()
                    .flex()
                    .h_full()
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
                    .px_4()
                    .flex()
                    .items_center()
                    .bg(colors.panel_background)
                    .border_t_1()
                    .border_color(colors.border)
                    .text_xs()
                    .text_color(activity_color(&self.activity, &colors))
                    .child(activity_label(&self.activity)),
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
                .child("Command-Shift-C  Edit commit subject")
                .child("Command-Shift-P  Command palette")
                .child("Command-/  Show or hide this reference")
                .child("Command-[ / Command-]  Back / Forward")
                .child("Up / Down  Move through loaded history")
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
                        div().flex_1().overflow_hidden().children(
                            commands
                                .into_iter()
                                .enumerate()
                                .map(|(row, (_, label, command))| {
                                    let selected_row = row == selected;
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
                                            app.run_palette_command(command, cx);
                                        }))
                                        .child(label)
                                        .into_any_element()
                                }),
                        ),
                    ),
            )
            .into_any_element()
    }

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
                    .child(
                        div()
                            .px_3()
                            .py_2()
                            .flex()
                            .gap_2()
                            .child(file_action_button(confirm_label, colors, cx, |app, cx| {
                                app.confirm_text_prompt(cx);
                            }))
                            .child(file_action_button("Cancel", colors, cx, |app, cx| {
                                app.cancel_text_prompt(cx);
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
                            .child(file_action_button("Merge", colors, cx, |app, cx| {
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
}
