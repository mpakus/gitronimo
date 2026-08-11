//! Configured remote browser.

use gpui::{SharedString, div, prelude::*, px};
use ui_kit::ThemeColors;

use crate::app_state::GitronimoApp;
use crate::views::components::{centered_empty_state, file_action_button};

impl GitronimoApp {
    #[allow(clippy::too_many_lines)]
    pub(crate) fn remotes_view(
        &self,
        colors: &ThemeColors,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let selected = self
            .refs
            .remotes
            .first()
            .and_then(|remote| String::from_utf8(remote.name.0.clone()).ok());
        let mut rows = Vec::new();
        for (index, remote) in self.refs.remotes.iter().enumerate() {
            let name = String::from_utf8_lossy(&remote.name.0).into_owned();
            let url = String::from_utf8_lossy(&remote.fetch_url).into_owned();
            let active = selected.as_deref() == Some(name.as_str());
            rows.push(
                div()
                    .id(SharedString::from(format!("remote-{index}")))
                    .h(px(44.0))
                    .px_3()
                    .flex()
                    .flex_col()
                    .justify_center()
                    .gap_0p5()
                    .border_b_1()
                    .border_color(colors.separator)
                    .bg(if active {
                        colors.accent
                    } else {
                        colors.panel_background
                    })
                    .child(
                        div()
                            .text_sm()
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .text_color(if active {
                                colors.panel_background
                            } else {
                                colors.text_primary
                            })
                            .child(name.clone()),
                    )
                    .child(
                        div()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .text_xs()
                            .text_color(if active {
                                colors.panel_background
                            } else {
                                colors.text_muted
                            })
                            .child(url.clone()),
                    )
                    .into_any_element(),
            );
        }
        let list = if rows.is_empty() {
            centered_empty_state(
                "No remotes",
                "Add a remote with Git to fetch and push changes.",
                colors,
            )
        } else {
            div().flex().flex_col().children(rows).into_any_element()
        };
        let detail = self.refs.remotes.first().map_or_else(
            || {
                centered_empty_state(
                    "No remote selected",
                    "Configure a remote to inspect its URL and fetch updates.",
                    colors,
                )
            },
            |remote| {
                let name = String::from_utf8_lossy(&remote.name.0).into_owned();
                let url = String::from_utf8_lossy(&remote.fetch_url).into_owned();
                let fetch_name = name.clone();
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .p_4()
                    .child(
                        div()
                            .text_lg()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child(name),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(colors.text_muted)
                            .child("Fetch URL"),
                    )
                    .child(div().text_sm().text_color(colors.text_secondary).child(url))
                    .child(file_action_button("Fetch", colors, cx, move |app, cx| {
                        app.fetch_remote(fetch_name.clone(), cx);
                    }))
                    .into_any_element()
            },
        );
        div()
            .flex()
            .flex_col()
            .h_full()
            .child(
                div()
                    .h(px(36.0))
                    .px_3()
                    .flex()
                    .items_center()
                    .border_b_1()
                    .border_color(colors.border)
                    .child(
                        div()
                            .text_sm()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child("Remotes"),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .flex()
                    .items_start()
                    .child(
                        div()
                            .w(px(280.0))
                            .h_full()
                            .border_r_1()
                            .border_color(colors.border)
                            .overflow_hidden()
                            .child(list),
                    )
                    .child(div().flex_1().h_full().overflow_hidden().child(detail)),
            )
    }
}
