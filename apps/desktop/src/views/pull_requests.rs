//! Pull Requests view: split list/detail browser and explicit remote actions.

use gpui::{AnyElement, ClickEvent, SharedString, div, prelude::*, px};
use ui_kit::ThemeColors;

use git_domain::PullRequestState;

use crate::app_state::GitronimoApp;
use crate::views::components::{
    centered_empty_state, detail_row, detail_section, file_action_button, two_pane_view,
    view_panel_header,
};

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
                        app.select_pull_request(index, cx);
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
                            .child(title),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(if selected {
                                colors.panel_background
                            } else {
                                colors.text_muted
                            })
                            .child(format!("{label} · {state}")),
                    )
                    .into_any_element(),
            );
        }
        let list = if rows.is_empty() {
            centered_empty_state(
                "No pull requests",
                "Open pull requests for the selected repository appear here.",
                colors,
            )
        } else {
            div().flex().flex_col().children(rows).into_any_element()
        };
        let detail = self.pull_request_detail.as_ref().map_or_else(
            || {
                centered_empty_state(
                    "No pull request selected",
                    "Choose a pull request to inspect changes and comments.",
                    colors,
                )
            },
            |detail| Self::pull_request_detail_view(detail, colors, cx),
        );
        let actions = div()
            .flex()
            .gap_1()
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
            ));
        two_pane_view(
            view_panel_header("Pull Requests", colors, Some(actions.into_any_element())),
            list,
            detail,
            colors,
        )
    }

    fn pull_request_detail_view(
        detail: &git_domain::PullRequestDetail,
        colors: &ThemeColors,
        cx: &mut gpui::Context<Self>,
    ) -> AnyElement {
        div()
            .flex()
            .flex_col()
            .gap_4()
            .p_4()
            .overflow_hidden()
            .child(
                div()
                    .text_lg()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .child(format!(
                        "#{} {}",
                        detail.summary.number, detail.summary.title
                    )),
            )
            .child(detail_section("Summary", colors))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .child(detail_row(
                        "Branch",
                        &format!("{} → {}", detail.summary.head_ref, detail.summary.base_ref),
                        colors,
                    ))
                    .child(detail_row(
                        "State",
                        pull_request_state_label(&detail.summary.state),
                        colors,
                    )),
            )
            .when(!detail.body.is_empty(), |panel| {
                panel.child(detail_section("Description", colors)).child(
                    div()
                        .text_sm()
                        .text_color(colors.text_secondary)
                        .child(detail.body.clone()),
                )
            })
            .child(detail_section("Changed files", colors))
            .children(detail.files.iter().map(|file| {
                detail_row(
                    &file.path,
                    &format!("+{} -{}", file.additions, file.deletions),
                    colors,
                )
            }))
            .child(detail_section("Comments", colors))
            .children(if detail.comments.is_empty() {
                vec![detail_row("Comments", "No comments yet.", colors)]
            } else {
                detail
                    .comments
                    .iter()
                    .map(|comment| {
                        detail_row(
                            &comment.author,
                            &format!("{} — {}", comment.created_at, comment.body),
                            colors,
                        )
                    })
                    .collect()
            })
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
