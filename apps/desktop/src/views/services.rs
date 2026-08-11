//! Hosting Services view: GitHub account state and hosted repositories.

use gpui::{AnyElement, ClickEvent, SharedString, div, prelude::*};
use ui_kit::ThemeColors;

use git_domain::ServiceAuthState;

use crate::app_state::{GitronimoApp, RepositoryView, ShellState};
use crate::views::components::file_action_button;

impl GitronimoApp {
    #[allow(clippy::too_many_lines)]
    pub(crate) fn services_view(
        &self,
        colors: &ThemeColors,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let mut rows: Vec<AnyElement> = Vec::new();
        for (index, repository) in self.hosted_repositories.iter().enumerate() {
            let selected = self.selected_hosted_repository == Some(index);
            let full_name = repository.full_name.clone();
            let clone_kind = if repository.private {
                "Private"
            } else {
                "Public"
            };
            rows.push(
                div()
                    .id(SharedString::from(format!("hosted-repository-{index}")))
                    .p_2()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .bg(if selected {
                        colors.selection
                    } else {
                        colors.panel_background
                    })
                    .border_1()
                    .border_color(colors.border)
                    .cursor_pointer()
                    .on_click(cx.listener(move |app, _: &ClickEvent, _, cx| {
                        app.selected_hosted_repository = Some(index);
                        cx.notify();
                    }))
                    .child(div().text_sm().child(full_name))
                    .child(
                        div()
                            .text_xs()
                            .text_color(colors.text_secondary)
                            .child(format!("{clone_kind} · Select to clone")),
                    )
                    .into_any_element(),
            );
        }
        let state_message = match &self.service_auth_state {
            ServiceAuthState::SignedOut => "No GitHub account connected.".to_owned(),
            ServiceAuthState::Loading => "Connecting to GitHub…".to_owned(),
            ServiceAuthState::Connected => "GitHub account connected.".to_owned(),
            ServiceAuthState::Expired => {
                "The saved GitHub token was rejected. Connect again.".to_owned()
            }
            ServiceAuthState::RateLimited => {
                "GitHub rate limit reached. Try again later.".to_owned()
            }
            ServiceAuthState::Error(message) => message.clone(),
        };
        let selected_repository = self
            .selected_hosted_repository
            .and_then(|index| self.hosted_repositories.get(index).cloned());
        let actions = match self.service_auth_state {
            ServiceAuthState::Connected => {
                let mut actions = div()
                    .flex()
                    .gap_2()
                    .child(file_action_button("Refresh", colors, cx, |app, cx| {
                        app.load_services(cx);
                    }))
                    .child(file_action_button("Sign out", colors, cx, |app, cx| {
                        app.sign_out_github(cx);
                    }))
                    .child(file_action_button(
                        "Clone selected…",
                        colors,
                        cx,
                        |app, cx| {
                            app.prompt_clone_hosted_repository(cx);
                        },
                    ));
                if let Some(repository) = selected_repository {
                    actions = actions.child(file_action_button(
                        "View pull requests",
                        colors,
                        cx,
                        move |app, cx| app.show_pull_requests(repository.clone(), cx),
                    ));
                }
                actions
            }
            _ => div()
                .flex()
                .gap_2()
                .child(file_action_button(
                    "Connect GitHub…",
                    colors,
                    cx,
                    |_, cx| GitronimoApp::prompt_connect_github(cx),
                ))
                .child(file_action_button("Refresh", colors, cx, |app, cx| {
                    app.load_services(cx);
                })),
        };
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(div().text_xl().child("Services"))
            .child(div().text_color(colors.text_secondary).child(state_message))
            .child(actions)
            .when(!matches!(self.state, ShellState::Welcome), |panel| {
                panel.child(file_action_button("Working Copy", colors, cx, |app, cx| {
                    app.navigate_to(RepositoryView::WorkingCopy, cx);
                }))
            })
            .children(
                if rows.is_empty() && self.service_auth_state == ServiceAuthState::Connected {
                    Some(
                        div()
                            .text_color(colors.text_muted)
                            .child("No repositories were returned for this account."),
                    )
                } else {
                    None
                },
            )
            .children(rows)
    }
}
