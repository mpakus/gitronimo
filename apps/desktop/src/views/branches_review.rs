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

    fn branch_needs_review(branch: &NamedRef) -> bool {
        branch.upstream.is_none() || branch.ahead > 0 || branch.behind > 0
    }

    fn review_branches(&self) -> Vec<&NamedRef> {
        let mut branches = self.sorted_local_branches();
        if !self.branches_review_show_all {
            branches.retain(|branch| Self::branch_needs_review(branch));
        }
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

    pub(crate) fn toggle_branches_review_filter(&mut self, cx: &mut gpui::Context<Self>) {
        self.branches_review_show_all = !self.branches_review_show_all;
        if let Some(name) = &self.selected_branch_review {
            let still_visible = self.review_branches().iter().any(|branch| {
                String::from_utf8(branch.name.0.clone()).ok().as_deref() == Some(name.as_str())
            });
            if !still_visible {
                self.selected_branch_review = None;
            }
        }
        cx.notify();
    }

    #[allow(clippy::too_many_lines)]
    pub(crate) fn branches_review_view(
        &self,
        colors: &ThemeColors,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let all_branches = self.sorted_local_branches();
        let branches = self.review_branches();
        let current_branch = self.current_branch_name();
        let selected = self.selected_branch_review.as_deref();

        let mut rows = Vec::new();
        for branch in &branches {
            let name = String::from_utf8(branch.name.0.clone()).unwrap_or_default();
            let active = selected == Some(name.as_str());
            let is_head = current_branch.as_deref() == Some(name.as_str());
            let badge = Self::branch_divergence_badge(branch);
            let select_name = name.clone();
            rows.push(
                div()
                    .id(SharedString::from(format!("branch-review-{name}")))
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
                        app.selected_branch_review = Some(select_name.clone());
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
                    .children(is_head.then(|| head_badge(colors, active)))
                    .children(badge.map(|text| count_badge(text, active, colors)))
                    .into_any_element(),
            );
        }

        let list = if rows.is_empty() {
            if !self.branches_review_show_all && !all_branches.is_empty() {
                centered_empty_state(
                    "No diverged branches",
                    "All local branches are in sync with their upstream. Use Show all to browse every branch.",
                    colors,
                )
            } else {
                centered_empty_state(
                    "No local branches",
                    "Create a branch from the sidebar or command palette to start tracking work.",
                    colors,
                )
            }
        } else {
            div().flex().flex_col().children(rows).into_any_element()
        };

        let detail = selected
            .and_then(|name| {
                branches
                    .iter()
                    .find(|branch| {
                        String::from_utf8(branch.name.0.clone()).ok().as_deref() == Some(name)
                    })
                    .copied()
            })
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
                                .children(is_head.then(|| head_badge(colors, false))),
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

        let toggle_label = if self.branches_review_show_all {
            "Show diverged only"
        } else {
            "Show all"
        };
        let header_actions = div()
            .flex()
            .gap_1()
            .child(file_action_button(toggle_label, colors, cx, |app, cx| {
                app.toggle_branches_review_filter(cx);
            }))
            .into_any_element();

        two_pane_view(
            view_panel_header("Branches Review", colors, Some(header_actions)),
            list,
            detail,
            colors,
        )
    }
}
