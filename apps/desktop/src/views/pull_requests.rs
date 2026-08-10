//! Pull Requests view: split list/detail browser and explicit remote actions.

use gpui::{AnyElement, ClickEvent, SharedString, div, prelude::*};
use ui_kit::ThemeColors;

use git_domain::PullRequestState;

use crate::app_state::GitronimoApp;
use crate::views::components::file_action_button;

impl GitronimoApp {
    #[allow(clippy::too_many_lines)]
    pub(crate) fn pull_requests_view(
        &self,
        colors: &ThemeColors,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let mut rows: Vec<AnyElement> = Vec::new();
        for (index, request) in self.pull_requests.iter().enumerate() {
            let selected = self.selected_pull_request == Some(index);
            let title = request.title.clone();
            let label = format!("#{} · {}", request.number, request.author);
            let state = pull_request_state_label(&request.state);
            rows.push(
                div()
                    .id(SharedString::from(format!("pull-request-{index}")))
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
                        app.select_pull_request(index, cx);
                    }))
                    .child(div().text_sm().child(title))
                    .child(
                        div()
                            .text_xs()
                            .text_color(colors.text_secondary)
                            .child(format!("{label} · {state}")),
                    )
                    .into_any_element(),
            );
        }
        let detail = self
            .pull_request_detail
            .as_ref()
            .map(|detail| Self::pull_request_detail_view(detail, colors, cx));
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(div().text_xl().child("Pull Requests"))
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(file_action_button("Services", colors, cx, |app, cx| {
                        app.show_services(cx);
                    }))
                    .child(file_action_button("Refresh", colors, cx, |app, cx| {
                        if let Some(repository) = app.pull_request_repository.clone() {
                            app.load_pull_requests(repository, cx);
                        }
                    }))
                    .child(file_action_button(
                        "New pull request…",
                        colors,
                        cx,
                        |app, cx| {
                            app.prompt_create_pull_request(cx);
                        },
                    )),
            )
            .child(
                div()
                    .flex()
                    .gap_3()
                    .child(
                        div()
                            .w_1_2()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .children(if rows.is_empty() {
                                vec![
                                    div()
                                        .text_color(colors.text_muted)
                                        .child("No open pull requests.")
                                        .into_any_element(),
                                ]
                            } else {
                                rows
                            }),
                    )
                    .child(
                        div()
                            .w_1_2()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .children(detail)
                            .when(self.pull_request_detail.is_none(), |this| {
                                this.child(
                                    div()
                                        .text_color(colors.text_muted)
                                        .child("Select a pull request to inspect it."),
                                )
                            }),
                    ),
            )
    }

    fn pull_request_detail_view(
        detail: &git_domain::PullRequestDetail,
        colors: &ThemeColors,
        cx: &mut gpui::Context<Self>,
    ) -> AnyElement {
        div()
            .p_3()
            .flex()
            .flex_col()
            .gap_2()
            .bg(colors.panel_background)
            .border_1()
            .border_color(colors.border)
            .child(div().text_lg().child(format!(
                "#{} {}",
                detail.summary.number, detail.summary.title
            )))
            .child(format!(
                "{} → {} · {}",
                detail.summary.head_ref,
                detail.summary.base_ref,
                pull_request_state_label(&detail.summary.state)
            ))
            .child(detail.body.clone())
            .child(
                div()
                    .text_sm()
                    .text_color(colors.text_secondary)
                    .child("Changed files"),
            )
            .children(detail.files.iter().map(|file| {
                div().font_family("Monaco").child(format!(
                    "{}  +{} -{}",
                    file.path, file.additions, file.deletions
                ))
            }))
            .child(
                div()
                    .text_sm()
                    .text_color(colors.text_secondary)
                    .child("Comments"),
            )
            .children(detail.comments.iter().map(|comment| {
                div()
                    .p_2()
                    .bg(colors.raised_background)
                    .child(format!("{} · {}", comment.author, comment.created_at))
                    .child(comment.body.clone())
            }))
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(file_action_button("Comment…", colors, cx, |app, cx| {
                        app.prompt_comment_pull_request(cx);
                    }))
                    .child(file_action_button("Merge…", colors, cx, |app, cx| {
                        app.prompt_merge_pull_request(cx);
                    }))
                    .child(file_action_button(
                        "Checkout branch",
                        colors,
                        cx,
                        |app, cx| {
                            app.checkout_pull_request(cx);
                        },
                    )),
            )
            .into_any_element()
    }
}

fn pull_request_state_label(state: &PullRequestState) -> &'static str {
    match state {
        PullRequestState::Open => "Open",
        PullRequestState::Closed => "Closed",
        PullRequestState::Merged => "Merged",
        PullRequestState::Other(_) => "Other",
    }
}
