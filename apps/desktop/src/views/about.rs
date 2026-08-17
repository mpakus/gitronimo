//! About `GitRonimo` overlay (application menu). Layout is approach-only from
//! typical two-column About windows; chrome and copy are original.

use gpui::{AnyElement, MouseButton, MouseDownEvent, div, img, prelude::*, px, rgb};
use ui_kit::ThemeColors;

use crate::app_state::GitronimoApp;

pub(crate) const APP_DISPLAY_NAME: &str = "GitRonimo";
pub(crate) const APP_SITE_URL: &str = "https://aomega.co";
pub(crate) const APP_SITE_LABEL: &str = "https://aomega.co";
pub(crate) const APP_TAGLINE: &str = "Made in Austin \u{2729} Texas";
/// Product version shown in About `GitRonimo`. Bump this after each release.
/// Keep in sync with `[package.metadata.packager] version` in `apps/desktop/Cargo.toml`.
/// Independent of the Cargo workspace version.
pub(crate) const APP_VERSION: &str = "2.0.4";

fn about_updates_copy(in_app_updates: bool) -> &'static str {
    if in_app_updates {
        "In-app updates are on. Check GitHub Releases for a newer notarized zip."
    } else {
        "In-app updates are off. Turn them on in GitRonimo → Settings…, then check."
    }
}

fn about_brand_column() -> impl IntoElement {
    div()
        .w(px(160.0))
        .flex_shrink_0()
        .flex()
        .flex_col()
        .items_center()
        .gap_3()
        .child(
            div()
                .size(px(96.0))
                .rounded(px(22.0))
                .overflow_hidden()
                .child(
                    img("icons/gitronimo-icon.png")
                        .size(px(96.0))
                        .flex_shrink_0(),
                ),
        )
        .child(
            div()
                .text_lg()
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(rgb(0xf2_f4_f7))
                .child(APP_DISPLAY_NAME),
        )
        .child(
            div()
                .text_xs()
                .text_color(rgb(0x9a_a3_ad))
                .child(format!("Version {APP_VERSION}")),
        )
}

fn about_check_updates_button(cx: &mut gpui::Context<GitronimoApp>) -> impl IntoElement {
    div()
        .id("about-check-updates")
        .debug_selector(|| "about-check-updates".into())
        .h(px(26.0))
        .px_3()
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(4.0))
        .bg(rgb(0x2a_2a_2a))
        .text_sm()
        .text_color(rgb(0xf2_f4_f7))
        .cursor_pointer()
        .hover(|style| style.bg(rgb(0x3a_3a_3a)))
        .on_click(cx.listener(|app, _, _, cx| {
            app.close_about_dialog_immediate(cx);
            app.check_for_app_updates(cx);
        }))
        .child("Check for updates")
}

impl GitronimoApp {
    fn about_details_column(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        div()
            .flex_1()
            .min_w(px(0.0))
            .overflow_hidden()
            .flex()
            .flex_col()
            .justify_center()
            .gap_3()
            .child(
                div()
                    .w_full()
                    .min_w(px(0.0))
                    .text_sm()
                    .text_color(rgb(0xf2_f4_f7))
                    .child(APP_TAGLINE),
            )
            .child(
                div()
                    .id("about-site-link")
                    .debug_selector(|| "about-site-link".into())
                    .w_full()
                    .min_w(px(0.0))
                    .text_sm()
                    .text_color(rgb(0x6a_b1_ff))
                    .cursor_pointer()
                    .hover(gpui::Styled::underline)
                    .on_click(cx.listener(|_, _, _, cx| {
                        cx.open_url(APP_SITE_URL);
                    }))
                    .child(APP_SITE_LABEL),
            )
            .child(
                div()
                    .w_full()
                    .min_w(px(0.0))
                    .text_xs()
                    .text_color(rgb(0x9a_a3_ad))
                    .child(about_updates_copy(self.in_app_updates)),
            )
            .child(about_check_updates_button(cx))
    }

    pub(crate) fn about_overlay(
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
                    app.close_about_dialog(cx);
                }),
            )
            .child(
                div()
                    .id("about-gitronimo")
                    .debug_selector(|| "about-gitronimo".into())
                    .w(px(520.0))
                    .overflow_hidden()
                    .p_6()
                    .flex()
                    .gap_6()
                    .bg(rgb(0x00_00_00))
                    .border_1()
                    .border_color(rgb(0x2a_2a_2a))
                    .rounded(px(12.0))
                    .shadow_lg()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|_, _: &MouseDownEvent, _, cx| {
                            cx.stop_propagation();
                        }),
                    )
                    .child(about_brand_column())
                    .child(self.about_details_column(cx)),
            )
            .into_any_element()
    }
}
