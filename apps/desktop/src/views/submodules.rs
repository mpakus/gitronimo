//! Submodules view: list the repository's submodules, update one, or open it
//! in Finder.

use gpui::{SharedString, div, prelude::*, px};
use ui_kit::ThemeColors;

use git_domain::WorktreeRepository;

use crate::app_state::{GitronimoApp, ShellState};
use crate::views::components::{
    centered_empty_state, file_action_button, two_pane_view, view_panel_header,
};

impl GitronimoApp {
    pub(crate) fn submodules_view(
        &self,
        _repository: &WorktreeRepository,
        colors: &ThemeColors,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let list_colors = *colors;
        let mut rows = Vec::new();
        for entry in &self.submodules {
            let path = String::from_utf8_lossy(&entry.path.0).to_string();
            let oid = String::from_utf8_lossy(&entry.oid).to_string();
            let (flag, state) = match entry.flag {
                b'-' => ("-", "uninitialized"),
                b'+' => ("+", "out of date"),
                b'U' => ("U", "conflicts"),
                _ => (" ", "clean"),
            };
            rows.push(
                div()
                    .id(SharedString::from(format!("submodule-{path}")))
                    .h(px(44.0))
                    .px_3()
                    .flex()
                    .flex_col()
                    .justify_center()
                    .gap_0p5()
                    .border_b_1()
                    .border_color(list_colors.separator)
                    .bg(list_colors.panel_background)
                    .child(div().text_sm().child(format!("{flag}  {path}  ({state})")))
                    .child(
                        div()
                            .text_xs()
                            .text_color(list_colors.text_muted)
                            .child(oid),
                    )
                    .into_any_element(),
            );
        }
        let list = if rows.is_empty() {
            centered_empty_state(
                "No submodules",
                "This repository does not declare any submodules.",
                colors,
            )
        } else {
            div().flex().flex_col().children(rows).into_any_element()
        };
        let detail = div()
            .flex()
            .flex_col()
            .gap_3()
            .p_4()
            .child(
                div()
                    .text_sm()
                    .text_color(colors.text_secondary)
                    .child(format!("{} submodule(s) configured", self.submodules.len())),
            )
            .child(file_action_button("Update all…", colors, cx, |_, cx| {
                GitronimoApp::prompt_submodule_update(None, cx);
            }))
            .into_any_element();
        let header_actions = div()
            .flex()
            .gap_1()
            .child(file_action_button("Refresh", colors, cx, |app, cx| {
                app.submodules_load_token = app.submodules_load_token.wrapping_add(1);
                if let ShellState::Repository(repository) = &app.state {
                    app.load_submodules(repository.clone(), cx);
                }
            }))
            .into_any_element();
        two_pane_view(
            view_panel_header("Submodules", colors, Some(header_actions)),
            list,
            detail,
            colors,
        )
    }
}
