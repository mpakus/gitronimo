//! The root window layout: toolbar, sidebar, content, activity bar.

use gpui::{AnyElement, MouseButton, MouseDownEvent, Render, Window, div, prelude::*, px};
use ui_kit::Theme;

use crate::app_state::{
    GitronimoApp, ShellState, ShortcutReferenceState, TextPromptKind, window_title,
};

use super::components::{
    activity_color, activity_label, error_view, file_action_button, loading_view,
};

impl Render for GitronimoApp {
    fn render(&mut self, window: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        window.set_window_title(&window_title(&self.state, self.has_commit_draft()));
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
                self.pending_text_prompt
                    .is_some()
                    .then(|| self.text_prompt_overlay(&colors, cx).into_any_element()),
            )
            .child(
                div()
                    .flex_1()
                    .flex()
                    .children(
                        matches!(self.state, ShellState::Welcome)
                            .then(|| self.welcome_vertical_rail(&colors, cx).into_any_element()),
                    )
                    .child(self.sidebar_view(sidebar_width, &colors, cx))
                    .child(
                        div()
                            .flex_1()
                            .h_full()
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
            .top(px(52.0))
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
        };
        div()
            .absolute()
            .top(px(52.0))
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
}
