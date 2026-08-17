//! Repository Settings view, plus the app-level Settings overlay (updates).

use gpui::{AnyElement, MouseButton, MouseDownEvent, div, prelude::*, px};
use ui_kit::ThemeColors;

use crate::actions::ShortcutReference;
use crate::app_state::{GitronimoApp, ThemeMode};
use crate::views::components::{
    detail_row, detail_section, file_action_button, file_action_button_named, view_panel_header,
};
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
                    .id("settings-scroll")
                    .flex_1()
                    .min_h(px(0.0))
                    .overflow_y_scroll()
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

    pub(crate) fn app_settings_overlay(
        &self,
        colors: &ThemeColors,
        cx: &mut gpui::Context<Self>,
    ) -> AnyElement {
        div()
            .absolute()
            .inset_0()
            .bg(colors.overlay_scrim)
            .flex()
            .items_center()
            .justify_center()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|app, _: &MouseDownEvent, _, cx| {
                    app.close_app_settings_dialog(cx);
                }),
            )
            .child(
                div()
                    .id("app-settings")
                    .debug_selector(|| "app-settings".into())
                    .w(px(520.0))
                    .max_h(px(420.0))
                    .overflow_y_scroll()
                    .p_6()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .bg(colors.panel_background)
                    .border_1()
                    .border_color(colors.border)
                    .rounded(px(12.0))
                    .shadow_lg()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|_, _: &MouseDownEvent, _, cx| {
                            cx.stop_propagation();
                        }),
                    )
                    .child(
                        div()
                            .text_lg()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(colors.text_primary)
                            .child("GitRonimo Settings"),
                    )
                    .child(self.updates_settings(colors, cx)),
            )
            .into_any_element()
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
                    .child(file_action_button_named(
                        "auto-stash-off",
                        "Off",
                        colors,
                        cx,
                        |app, cx| {
                            app.set_auto_stash(false, cx);
                        },
                    ))
                    .child(file_action_button_named(
                        "auto-stash-on",
                        "On",
                        colors,
                        cx,
                        |app, cx| {
                            app.set_auto_stash(true, cx);
                        },
                    )),
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
            .child(detail_row(
                "Installed version",
                crate::views::about::APP_VERSION,
                colors,
            ))
            .child(detail_row("In-app updates", state, colors))
            .child(
                div()
                    .flex()
                    .gap_1()
                    .child(file_action_button_named(
                        "updates-off",
                        "Off",
                        colors,
                        cx,
                        |app, cx| {
                            app.set_in_app_updates(false, cx);
                        },
                    ))
                    .child(file_action_button_named(
                        "updates-on",
                        "On",
                        colors,
                        cx,
                        |app, cx| {
                            app.set_in_app_updates(true, cx);
                        },
                    ))
                    .child(file_action_button_named(
                        "updates-check-now",
                        "Check now",
                        colors,
                        cx,
                        |app, cx| {
                            app.check_for_app_updates(cx);
                        },
                    )),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(colors.text_muted)
                    .child(
                        "On by default. Check now reads GitHub Releases for a newer notarized zip, verifies SHA-256 and Gatekeeper, then replaces this .app. No check on launch. No telemetry.",
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
                    .child(file_action_button_named(
                        "ai-commit-off",
                        "Off",
                        colors,
                        cx,
                        |app, cx| {
                            app.set_ai_commit_messages(false, cx);
                        },
                    ))
                    .child(file_action_button_named(
                        "ai-commit-on",
                        "On",
                        colors,
                        cx,
                        |app, cx| {
                            app.set_ai_commit_messages(true, cx);
                        },
                    )),
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
