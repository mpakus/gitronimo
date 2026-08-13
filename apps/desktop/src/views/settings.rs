//! Dedicated Settings view: appearance, identity, and shortcuts.

use gpui::{div, prelude::*, px};
use ui_kit::ThemeColors;

use crate::actions::ShortcutReference;
use crate::app_state::{GitronimoApp, ThemeMode};
use crate::views::components::{detail_row, detail_section, file_action_button, view_panel_header};
use git_domain::ServiceAuthState;

impl GitronimoApp {
    pub(crate) fn settings_view(
        &self,
        colors: &ThemeColors,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let appearance_label = match self.theme_mode {
            ThemeMode::System => "System",
            ThemeMode::Light => "Light",
            ThemeMode::Dark => "Dark",
        };
        let github_account = self
            .service_account
            .as_ref()
            .map_or_else(|| "Not connected".into(), |account| account.login.clone());
        let connected = self.service_auth_state == ServiceAuthState::Connected;
        div()
            .flex()
            .flex_col()
            .h_full()
            .child(view_panel_header("Settings", colors, None))
            .child(
                div()
                    .flex_1()
                    .overflow_hidden()
                    .p_4()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .max_w(px(520.0))
                    .child(detail_section("APPEARANCE", colors))
                    .child(detail_row("Theme", appearance_label, colors))
                    .child(
                        div()
                            .flex()
                            .gap_1()
                            .child(file_action_button("System", colors, cx, |app, cx| {
                                app.theme_mode = ThemeMode::System;
                                app.apply_theme_mode(None, cx);
                            }))
                            .child(file_action_button("Light", colors, cx, |app, cx| {
                                app.theme_mode = ThemeMode::Light;
                                app.apply_theme_mode(None, cx);
                            }))
                            .child(file_action_button("Dark", colors, cx, |app, cx| {
                                app.theme_mode = ThemeMode::Dark;
                                app.apply_theme_mode(None, cx);
                            })),
                    )
                    .child(detail_section("GIT IDENTITY", colors))
                    .child(detail_row("Committer", &self.author_identity, colors))
                    .child(detail_section("GITHUB", colors))
                    .child(detail_row("Account", &github_account, colors))
                    .child(if connected {
                        div()
                            .flex()
                            .gap_1()
                            .child(file_action_button("Refresh", colors, cx, |app, cx| {
                                app.load_github_account(cx);
                            }))
                            .child(file_action_button("Sign out", colors, cx, |app, cx| {
                                app.sign_out_github(cx);
                            }))
                            .into_any_element()
                    } else {
                        file_action_button("Connect GitHub…", colors, cx, |_, cx| {
                            GitronimoApp::prompt_connect_github(cx);
                        })
                        .into_any_element()
                    })
                    .child(detail_section("KEYBOARD", colors))
                    .child(file_action_button(
                        "Show Shortcuts…",
                        colors,
                        cx,
                        |app, cx| {
                            app.shortcut_reference_state =
                                crate::app_state::ShortcutReferenceState::Visible;
                            cx.notify();
                        },
                    ))
                    .on_action(cx.listener(
                        |_: &mut GitronimoApp, _: &ShortcutReference, _, cx| {
                            cx.notify();
                        },
                    )),
            )
    }
}
