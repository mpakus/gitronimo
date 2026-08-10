//! Configured remote browser.

use gpui::{SharedString, div, prelude::*};
use ui_kit::ThemeColors;

use crate::app_state::{GitronimoApp, RepositoryView};
use crate::views::components::file_action_button;

impl GitronimoApp {
    pub(crate) fn remotes_view(
        &self,
        colors: &ThemeColors,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(div().text_xl().child("Remotes"))
            .child(file_action_button("Working Copy", colors, cx, |app, cx| {
                app.navigate_to(RepositoryView::WorkingCopy, cx);
            }))
            .children(if self.refs.remotes.is_empty() {
                Some(
                    div()
                        .text_color(colors.text_muted)
                        .child("No configured remotes.")
                        .into_any_element(),
                )
            } else {
                None
            })
            .children(self.refs.remotes.iter().enumerate().map(|(index, remote)| {
                let name = String::from_utf8_lossy(&remote.name.0).into_owned();
                let url = String::from_utf8_lossy(&remote.fetch_url).into_owned();
                let fetch_name = name.clone();
                div()
                    .id(SharedString::from(format!("remote-{index}")))
                    .p_2()
                    .flex()
                    .items_center()
                    .justify_between()
                    .bg(colors.panel_background)
                    .border_1()
                    .border_color(colors.border)
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(name)
                            .child(div().text_sm().text_color(colors.text_secondary).child(url)),
                    )
                    .child(file_action_button("Fetch", colors, cx, move |app, cx| {
                        app.fetch_remote(fetch_name.clone(), cx);
                    }))
            }))
    }
}
