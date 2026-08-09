//! Submodules view: list the repository's submodules, update one, or open it
//! in Finder.

use gpui::{SharedString, div, prelude::*};
use ui_kit::ThemeColors;

use git_domain::WorktreeRepository;

use crate::app_state::{GitronimoApp, RepositoryView, ShellState};
use crate::views::components::file_action_button;

impl GitronimoApp {
    pub(crate) fn submodules_view(
        &self,
        _repository: &WorktreeRepository,
        colors: &ThemeColors,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let list_colors = *colors;
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(div().text_xl().child("Submodules"))
            .child(file_action_button("Working Copy", colors, cx, |app, cx| {
                app.navigate_to(RepositoryView::WorkingCopy, cx);
            }))
            .child(file_action_button("Update all…", colors, cx, |_, cx| {
                GitronimoApp::prompt_submodule_update(None, cx);
            }))
            .child(file_action_button(
                "Refresh submodules",
                colors,
                cx,
                |app, cx| {
                    app.submodules_load_token = app.submodules_load_token.wrapping_add(1);
                    if let ShellState::Repository(repository) = &app.state {
                        app.load_submodules(repository.clone(), cx);
                    }
                },
            ))
            .children(self.submodules.iter().map(|entry| {
                let path = String::from_utf8_lossy(&entry.path.0).to_string();
                let oid = String::from_utf8_lossy(&entry.oid).to_string();
                let update_path = entry.path.clone();
                let open_path = entry.path.clone();
                let (flag, state) = match entry.flag {
                    b'-' => ("-", "uninitialized"),
                    b'+' => ("+", "out of date"),
                    b'U' => ("U", "conflicts"),
                    _ => (" ", "clean"),
                };
                div()
                    .id(SharedString::from(format!("submodule-{path}")))
                    .px_2()
                    .py_1()
                    .flex()
                    .flex_col()
                    .bg(list_colors.panel_background)
                    .border_b_1()
                    .border_color(list_colors.border)
                    .child(format!("{flag}  {path}  ({state})"))
                    .child(oid)
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(file_action_button("Update…", colors, cx, move |_, cx| {
                                GitronimoApp::prompt_submodule_update(
                                    Some(update_path.clone()),
                                    cx,
                                );
                            }))
                            .child(file_action_button("Open", colors, cx, move |_, cx| {
                                GitronimoApp::prompt_open_submodule(open_path.clone(), cx);
                            })),
                    )
            }))
    }
}
