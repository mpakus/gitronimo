//! Hosting Services view: GitHub account state and hosted repositories.

use gpui::{AnyElement, ClickEvent, SharedString, div, prelude::*, px};
use ui_kit::ThemeColors;

use git_domain::ServiceAuthState;

use crate::app_state::{GitronimoApp, RepositoryView, ShellState};
use crate::views::components::{
    centered_empty_state, detail_row, detail_section, file_action_button, two_pane_view,
    view_panel_header,
};

impl GitronimoApp {
    #[allow(clippy::too_many_lines)]
    pub(crate) fn services_view(
        &self,
        colors: &ThemeColors,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let state_message = match &self.service_auth_state {
            ServiceAuthState::SignedOut => "No GitHub account connected.",
            ServiceAuthState::Loading => "Connecting to GitHub…",
            ServiceAuthState::Connected => "GitHub account connected.",
            ServiceAuthState::Expired => "The saved GitHub token was rejected. Connect again.",
            ServiceAuthState::RateLimited => "GitHub rate limit reached. Try again later.",
            ServiceAuthState::Error(message) => message.as_str(),
        };
        let account_login = self
            .service_account
            .as_ref()
            .map_or_else(|| "Not connected".into(), |account| account.login.clone());
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
                    .h(px(44.0))
                    .px_3()
                    .flex()
                    .flex_col()
                    .justify_center()
                    .gap_0p5()
                    .border_b_1()
                    .border_color(colors.separator)
                    .bg(if selected {
                        colors.accent
                    } else {
                        colors.panel_background
                    })
                    .cursor_pointer()
                    .on_click(cx.listener(move |app, _: &ClickEvent, _, cx| {
                        app.selected_hosted_repository = Some(index);
                        cx.notify();
                    }))
                    .child(
                        div()
                            .text_sm()
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .text_color(if selected {
                                colors.panel_background
                            } else {
                                colors.text_primary
                            })
                            .child(full_name),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(if selected {
                                colors.panel_background
                            } else {
                                colors.text_muted
                            })
                            .child(clone_kind),
                    )
                    .into_any_element(),
            );
        }
        let list = if rows.is_empty() {
            centered_empty_state(
                "No hosted repositories",
                if self.service_auth_state == ServiceAuthState::Connected {
                    "No repositories were returned for this account."
                } else {
                    "Connect a GitHub account to browse remote repositories."
                },
                colors,
            )
        } else {
            div().flex().flex_col().children(rows).into_any_element()
        };
        let detail = self
            .selected_hosted_repository
            .and_then(|index| self.hosted_repositories.get(index))
            .map_or_else(
                || {
                    centered_empty_state(
                        "No repository selected",
                        "Choose a hosted repository to clone or inspect pull requests.",
                        colors,
                    )
                },
                |repository| {
                    let repo_for_prs = repository.clone();
                    div()
                        .flex()
                        .flex_col()
                        .gap_4()
                        .p_4()
                        .child(
                            div()
                                .text_lg()
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .child(repository.full_name.clone()),
                        )
                        .child(detail_section("Repository", colors))
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .child(detail_row(
                                    "Visibility",
                                    if repository.private {
                                        "Private"
                                    } else {
                                        "Public"
                                    },
                                    colors,
                                ))
                                .child(detail_row("Clone URL", &repository.clone_url, colors)),
                        )
                        .child(
                            div()
                                .flex()
                                .gap_2()
                                .child(file_action_button(
                                    "Clone selected…",
                                    colors,
                                    cx,
                                    |app, cx| {
                                        app.prompt_clone_hosted_repository(cx);
                                    },
                                ))
                                .child(file_action_button(
                                    "View pull requests",
                                    colors,
                                    cx,
                                    move |app, cx| {
                                        app.show_pull_requests(repo_for_prs.clone(), cx);
                                    },
                                )),
                        )
                        .into_any_element()
                },
            );
        let mut actions =
            div()
                .flex()
                .gap_1()
                .child(file_action_button("Refresh", colors, cx, |app, cx| {
                    app.load_services(cx);
                }));
        if self.service_auth_state == ServiceAuthState::Connected {
            actions = actions.child(file_action_button("Sign out", colors, cx, |app, cx| {
                app.sign_out_github(cx);
            }));
        } else {
            actions = actions.child(file_action_button(
                "Connect GitHub…",
                colors,
                cx,
                |_, cx| GitronimoApp::prompt_connect_github(cx),
            ));
        }
        if !matches!(self.state, ShellState::Welcome) {
            actions = actions.child(file_action_button("Working Copy", colors, cx, |app, cx| {
                app.navigate_to(RepositoryView::WorkingCopy, cx);
            }));
        }
        let account_panel = div()
            .px_4()
            .py_3()
            .border_b_1()
            .border_color(colors.separator)
            .flex()
            .flex_col()
            .gap_2()
            .child(detail_section("Account", colors))
            .child(detail_row("Login", &account_login, colors))
            .child(detail_row("Status", state_message, colors))
            .child(actions);
        two_pane_view(
            view_panel_header("Services", colors, None),
            div()
                .flex()
                .flex_col()
                .h_full()
                .child(account_panel)
                .child(div().flex_1().overflow_hidden().child(list))
                .into_any_element(),
            detail,
            colors,
        )
    }
}
