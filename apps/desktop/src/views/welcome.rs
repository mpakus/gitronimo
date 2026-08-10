//! Repositories view: a compact local repository browser and detail surface.

use gpui::{AnyElement, div, prelude::*, px};
use ui_kit::ThemeColors;

use crate::actions::OpenRepository;
use crate::app_state::GitronimoApp;
use crate::views::components::{file_action_button, primary_window_action_button};

impl GitronimoApp {
    #[allow(clippy::unused_self)]
    pub(crate) fn welcome_view(
        &self,
        colors: &ThemeColors,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let content = welcome_drop_zone(colors, cx);
        div().flex_1().flex().flex_col().child(content)
    }
}

fn welcome_drop_zone(colors: &ThemeColors, cx: &mut gpui::Context<GitronimoApp>) -> AnyElement {
    div()
        .flex_1()
        .flex()
        .items_center()
        .justify_center()
        .child(
            div()
                .flex()
                .flex_col()
                .items_center()
                .gap_4()
                .child(
                    div()
                        .w(px(260.0))
                        .h(px(260.0))
                        .flex()
                        .flex_col()
                        .items_center()
                        .justify_center()
                        .border_2()
                        .border_dashed()
                        .border_color(colors.border)
                        .rounded(px(8.0))
                        .gap_4()
                        .child(div().text_3xl().child("\u{1F4C1}"))
                        .child(
                            div()
                                .text_base()
                                .text_color(colors.text_muted)
                                .text_center()
                                .child("Drop Folder or URL\nto Add Git Repository"),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .gap_2()
                        .child(primary_window_action_button(
                            "Add",
                            colors,
                            cx,
                            |_, window, cx| {
                                window.dispatch_action(Box::new(OpenRepository), cx);
                            },
                        ))
                        .child(file_action_button("Create", colors, cx, |app, cx| {
                            app.prompt_create_repository(cx);
                        }))
                        .child(file_action_button("Clone", colors, cx, |app, cx| {
                            app.prompt_clone_repository(cx);
                        })),
                )
                .into_any_element(),
        )
        .into_any_element()
}
