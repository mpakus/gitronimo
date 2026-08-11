//! Branches Review — compare local branches against their upstream tracking refs.

use git_domain::NamedRef;
use gpui::{ClickEvent, SharedString, div, prelude::*, px};
use ui_kit::ThemeColors;

use crate::app_state::GitronimoApp;
use crate::views::components::{
    LIST_ROW_HEIGHT, centered_empty_state, count_badge, detail_row, file_action_button, head_badge,
    two_pane_view, view_panel_header,
};

impl GitronimoApp {
    fn current_branch_name(&self) -> Option<String> {
        self.working_copy
            .as_ref()
            .and_then(|status| match &status.branch.head {
                git_domain::HeadStatus::Branch(branch) => String::from_utf8(branch.0.clone()).ok(),
                _ => None,
            })
    }

    fn sorted_local_branches(&self) -> Vec<&NamedRef> {
        let mut branches: Vec<_> = self
            .refs
            .local_branches
            .iter()
            .filter(|branch| String::from_utf8(branch.name.0.clone()).is_ok())
            .collect();
        branches.sort_by(|left, right| {
            String::from_utf8(left.name.0.clone())
                .unwrap_or_default()
                .cmp(&String::from_utf8(right.name.0.clone()).unwrap_or_default())
        });
        branches
    }

    fn branch_divergence_badge(branch: &NamedRef) -> Option<String> {
        if branch.upstream.is_none() {
            return Some("unpublished".into());
        }
        if branch.ahead == 0 && branch.behind == 0 {
            return None;
        }
        let mut parts = Vec::new();
        if branch.ahead > 0 {
            parts.push(format!("{} \u{2191}", branch.ahead));
        }
        if branch.behind > 0 {
            parts.push(format!("{} \u{2193}", branch.behind));
        }
        Some(parts.join(" "))
    }

    fn format_branch_tracking(branch: &NamedRef) -> String {
        match &branch.upstream {
            Some(upstream) if branch.ahead == 0 && branch.behind == 0 => {
                format!("In sync with {upstream}")
            }
            Some(upstream) => {
                let mut parts = vec![upstream.clone()];
                if branch.ahead > 0 {
                    parts.push(format!("{} ahead", branch.ahead));
                }
                if branch.behind > 0 {
                    parts.push(format!("{} behind", branch.behind));
                }
                parts.join(" · ")
            }
            None => "No upstream configured".into(),
        }
    }

    #[allow(clippy::too_many_lines)]
    pub(crate) fn branches_review_view(
        &self,
        colors: &ThemeColors,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let branches = self.sorted_local_branches();
        let current_branch = self.current_branch_name();
        let selected = self
            .selected_branch_review
            .filter(|index| branches.get(*index).is_some());

        let mut rows = Vec::new();
        for (index, branch) in branches.iter().enumerate() {
            let name = String::from_utf8(branch.name.0.clone()).unwrap_or_default();
            let active = selected == Some(index);
            let is_head = current_branch.as_deref() == Some(name.as_str());
            let badge = Self::branch_divergence_badge(branch);
            rows.push(
                div()
                    .id(SharedString::from(format!("branch-review-{index}")))
                    .h(px(LIST_ROW_HEIGHT))
                    .px_3()
                    .flex()
                    .items_center()
                    .gap_2()
                    .border_b_1()
                    .border_color(colors.list_row_border)
                    .bg(if active {
                        colors.accent
                    } else {
                        colors.panel_background
                    })
                    .cursor_pointer()
                    .on_click(cx.listener(move |app, _: &ClickEvent, _, cx| {
                        app.selected_branch_review = Some(index);
                        cx.notify();
                    }))
                    .child(
                        div()
                            .flex_1()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .text_sm()
                            .text_color(if active {
                                colors.panel_background
                            } else {
                                colors.text_primary
                            })
                            .child(name.clone()),
                    )
                    .children(is_head.then(|| head_badge(colors)))
                    .children(badge.map(|text| count_badge(text, active, colors)))
                    .into_any_element(),
            );
        }

        let list = if rows.is_empty() {
            centered_empty_state(
                "No local branches",
                "Create a branch from the sidebar or command palette to start tracking work.",
                colors,
            )
        } else {
            div().flex().flex_col().children(rows).into_any_element()
        };

        let detail = selected
            .and_then(|index| branches.get(index).copied())
            .map_or_else(
                || {
                    centered_empty_state(
                        "Select a branch",
                        "Choose a branch to inspect its upstream tracking and divergence.",
                        colors,
                    )
                },
                |branch| {
                    let name = String::from_utf8(branch.name.0.clone()).unwrap_or_default();
                    let checkout_name = name.clone();
                    let is_head = current_branch.as_deref() == Some(name.as_str());
                    let short_oid = branch.target.chars().take(8).collect::<String>();
                    div()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .p_4()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_2()
                                .child(
                                    div()
                                        .text_lg()
                                        .font_weight(gpui::FontWeight::SEMIBOLD)
                                        .child(name.clone()),
                                )
                                .children(is_head.then(|| head_badge(colors))),
                        )
                        .child(detail_row("Commit", &short_oid, colors))
                        .child(detail_row(
                            "Tracking",
                            &Self::format_branch_tracking(branch),
                            colors,
                        ))
                        .children((!is_head).then(|| {
                            file_action_button("Checkout", colors, cx, move |app, cx| {
                                app.checkout_branch(checkout_name.clone(), cx);
                            })
                        }))
                        .into_any_element()
                },
            );

        two_pane_view(
            view_panel_header("Branches Review", colors, None),
            list,
            detail,
            colors,
        )
    }
}
