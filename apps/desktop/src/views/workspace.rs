//! The root window layout: toolbar, sidebar, content, inspector, activity bar.

use gpui::{AnyElement, Render, Window, div, prelude::*, px};
use ui_kit::Theme;

use crate::app_state::{GitronimoApp, ShellState, ShortcutReferenceState, window_title};

use super::components::{
    activity_color, activity_label, error_view, file_action_button, loading_view,
};

impl Render for GitronimoApp {
    fn render(&mut self, window: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        window.set_window_title(&window_title(&self.state, self.has_commit_draft()));
        let colors = Theme::for_appearance(self.appearance).colors;
        let sidebar_width = self.sidebar_width;
        let inspector_width = self.inspector_width;
        let show_inspector = !matches!(self.state, ShellState::Welcome)
            && crate::app_state::shows_inspector(
                f32::from(window.viewport_size().width),
                sidebar_width,
                inspector_width,
            );
        let content = match &self.state {
            ShellState::Welcome => self.welcome_view(&colors, cx).into_any_element(),
            ShellState::Loading(path) => loading_view(path, &colors).into_any_element(),
            ShellState::Repository(repository) => self
                .repository_view(repository, &colors, cx)
                .into_any_element(),
            ShellState::Error(message) => error_view(message, &colors).into_any_element(),
        };

        div()
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
            .on_action(cx.listener(Self::widen_inspector))
            .on_drop(cx.listener(Self::dropped_paths))
            .child(self.workspace_toolbar(&colors, cx))
            .child(
                div()
                    .flex_1()
                    .flex()
                    .child(self.sidebar_view(sidebar_width, &colors, cx))
                    .child(
                        div()
                            .flex_1()
                            .h_full()
                            .p_6()
                            .child(content)
                            .children(self.shortcut_reference_view(&colors, cx)),
                    )
                    .when(show_inspector, |this| {
                        this.child(
                            div()
                                .w(px(inspector_width))
                                .h_full()
                                .p_4()
                                .bg(colors.panel_background)
                                .border_l_1()
                                .border_color(colors.border)
                                .child("Diagnostics")
                                .child(self.diagnostics.clone())
                                .child("One repository window opens per selection."),
                        )
                    }),
            )
            .child(
                div()
                    .min_h(px(30.0))
                    .px_4()
                    .flex()
                    .items_center()
                    .bg(colors.raised_background)
                    .border_t_1()
                    .border_color(colors.border)
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
}
