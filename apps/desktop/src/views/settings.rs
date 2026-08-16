//! Dedicated Settings view: appearance, Git engine, stashing, updates, AI commits, identity, GitHub, and shortcuts.

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
                    .child(self.git_engine_settings(colors, cx))
                    .child(self.auto_stash_settings(colors, cx))
                    .child(self.updates_settings(colors, cx))
                    .child(self.ai_commit_settings(colors, cx))
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

    fn git_engine_settings(
        &self,
        colors: &ThemeColors,
        cx: &mut gpui::Context<Self>,
    ) -> gpui::AnyElement {
        let backend = if self.use_system_git {
            "System Git"
        } else {
            "gix"
        };
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(detail_section("GIT ENGINE", colors))
            .child(detail_row("Backend", backend, colors))
            .child(
                div()
                    .flex()
                    .gap_1()
                    .child(file_action_button("gix", colors, cx, |app, cx| {
                        app.set_use_system_git(false, cx);
                    }))
                    .child(file_action_button("System Git", colors, cx, |app, cx| {
                        app.set_use_system_git(true, cx);
                    })),
            )
            .into_any_element()
    }

    fn auto_stash_settings(
        &self,
        colors: &ThemeColors,
        cx: &mut gpui::Context<Self>,
    ) -> gpui::AnyElement {
        let state = if self.auto_stash { "On" } else { "Off" };
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(detail_section("STASHING", colors))
            .child(detail_row("Auto-stash before switch and pull", state, colors))
            .child(
                div()
                    .flex()
                    .gap_1()
                    .child(file_action_button("Off", colors, cx, |app, cx| {
                        app.set_auto_stash(false, cx);
                    }))
                    .child(file_action_button("On", colors, cx, |app, cx| {
                        app.set_auto_stash(true, cx);
                    })),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(colors.text_muted)
                    .child(
                        "When on, dirty work is stashed, the switch or pull runs, then the stash is reapplied. Off by default.",
                    ),
            )
            .into_any_element()
    }

    fn updates_settings(
        &self,
        colors: &ThemeColors,
        cx: &mut gpui::Context<Self>,
    ) -> gpui::AnyElement {
        let state = if self.in_app_updates { "On" } else { "Off" };
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(detail_section("UPDATES", colors))
            .child(detail_row("In-app updates", state, colors))
            .child(
                div()
                    .flex()
                    .gap_1()
                    .child(file_action_button("Off", colors, cx, |app, cx| {
                        app.set_in_app_updates(false, cx);
                    }))
                    .child(file_action_button("On", colors, cx, |app, cx| {
                        app.set_in_app_updates(true, cx);
                    }))
                    .child(file_action_button("Check now", colors, cx, |app, cx| {
                        app.check_for_app_updates(cx);
                    })),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(colors.text_muted)
                    .child(
                        "Off by default. When on, Check now downloads the notarized GitHub zip, verifies SHA-256 and Gatekeeper, then replaces this .app. No check on launch. No telemetry.",
                    ),
            )
            .into_any_element()
    }

    fn ai_commit_settings(
        &self,
        colors: &ThemeColors,
        cx: &mut gpui::Context<Self>,
    ) -> gpui::AnyElement {
        let state = if self.ai_commit_messages { "On" } else { "Off" };
        let endpoint = if self.ai_commit_endpoint.is_empty() {
            app_core::DEFAULT_AI_COMMIT_ENDPOINT
        } else {
            self.ai_commit_endpoint.as_str()
        };
        let model = if self.ai_commit_model.is_empty() {
            app_core::DEFAULT_AI_COMMIT_MODEL
        } else {
            self.ai_commit_model.as_str()
        };
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(detail_section("AI COMMIT MESSAGES", colors))
            .child(detail_row("Suggestions", state, colors))
            .child(detail_row("Endpoint", endpoint, colors))
            .child(detail_row("Model", model, colors))
            .child(
                div()
                    .flex()
                    .gap_1()
                    .child(file_action_button("Off", colors, cx, |app, cx| {
                        app.set_ai_commit_messages(false, cx);
                    }))
                    .child(file_action_button("On", colors, cx, |app, cx| {
                        app.set_ai_commit_messages(true, cx);
                    })),
            )
            .child(
                div()
                    .flex()
                    .gap_1()
                    .child(file_action_button("Endpoint…", colors, cx, |app, cx| {
                        app.prompt_ai_commit_endpoint(cx);
                    }))
                    .child(file_action_button("Model…", colors, cx, |app, cx| {
                        app.prompt_ai_commit_model(cx);
                    }))
                    .child(file_action_button("API key…", colors, cx, |_, cx| {
                        GitronimoApp::prompt_ai_commit_key(cx);
                    }))
                    .child(file_action_button("Clear key", colors, cx, |_, cx| {
                        GitronimoApp::clear_ai_commit_key(cx);
                    })),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(colors.text_muted)
                    .child(
                        "Off by default. Suggest uses only the staged diff you can see. The reply fills the composer; you still edit and commit. HTTPS remotes need a Keychain API key. Localhost HTTP is allowed for a local model. No telemetry.",
                    ),
            )
            .into_any_element()
    }
}
