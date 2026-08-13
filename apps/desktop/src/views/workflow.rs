//! Workflow tab: templates, applied bases/topics, Start / Finish / Sync.
//!
//! Approach-only from Tower Workflows (GitHub Flow / GitLab Flow / git-flow).
//! Git mutations stay in `main.rs`; this module only renders and dispatches.

use gpui::{AnyElement, ClickEvent, SharedString, div, prelude::*, px};
use ui_kit::ThemeColors;

use app_core::{RepositoryWorkflow, WorkflowKind};

use crate::app_state::{GitronimoApp, ShellState};
use crate::views::components::{
    centered_empty_state, file_action_button, two_pane_view, view_panel_header,
};

impl GitronimoApp {
    #[allow(clippy::too_many_lines)]
    pub(crate) fn workflow_view(
        &self,
        colors: &ThemeColors,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let applied = self.workflow.as_ref();
        let git_actions = matches!(self.state, ShellState::Repository(_));
        let mut rows = Vec::new();
        for (kind, label) in [
            (WorkflowKind::GitHubFlow, "GitHub Flow"),
            (WorkflowKind::GitLabFlow, "GitLab Flow"),
            (WorkflowKind::GitFlow, "git-flow"),
        ] {
            let active = applied.is_some_and(|workflow| workflow.kind == kind);
            rows.push(template_row(
                SharedString::from(format!("workflow-template-{label}")),
                label,
                kind.caption(),
                active,
                colors,
                cx,
                move |app, cx| app.apply_workflow_kind(kind, cx),
            ));
        }
        rows.push(template_row(
            SharedString::from("workflow-template-detect"),
            "Auto-detect",
            "Infer a template from existing local branch names.",
            false,
            colors,
            cx,
            GitronimoApp::detect_workflow,
        ));
        if applied.is_some() {
            rows.push(template_row(
                SharedString::from("workflow-template-disable"),
                "Disable",
                "Clear the saved convention for this repository.",
                false,
                colors,
                cx,
                GitronimoApp::disable_workflow,
            ));
        }
        let list = div().flex().flex_col().children(rows).into_any_element();
        let detail = applied.map_or_else(
            || {
                centered_empty_state(
                    "No workflow",
                    "Choose GitHub Flow, GitLab Flow, git-flow, or Auto-detect.",
                    colors,
                )
            },
            |workflow| workflow_detail(self, workflow, git_actions, colors, cx),
        );
        two_pane_view(
            view_panel_header("Workflow", colors, None),
            list,
            detail,
            colors,
        )
    }
}

fn template_row(
    id: SharedString,
    title: &'static str,
    caption: &'static str,
    active: bool,
    colors: &ThemeColors,
    cx: &mut gpui::Context<GitronimoApp>,
    on_click: impl Fn(&mut GitronimoApp, &mut gpui::Context<GitronimoApp>) + 'static,
) -> AnyElement {
    div()
        .id(id)
        .px_3()
        .py_2()
        .flex()
        .flex_col()
        .gap_0p5()
        .border_b_1()
        .border_color(colors.separator)
        .bg(if active {
            colors.accent
        } else {
            colors.panel_background
        })
        .cursor_pointer()
        .when(!active, |row| row.hover(|style| style.bg(colors.selection)))
        .on_click(cx.listener(move |app, _: &ClickEvent, _, cx| on_click(app, cx)))
        .child(
            div()
                .text_sm()
                .font_weight(gpui::FontWeight::MEDIUM)
                .text_color(if active {
                    colors.panel_background
                } else {
                    colors.text_primary
                })
                .child(title),
        )
        .child(
            div()
                .text_xs()
                .text_color(if active {
                    colors.panel_background
                } else {
                    colors.text_muted
                })
                .child(caption),
        )
        .into_any_element()
}

#[allow(clippy::too_many_lines)]
fn workflow_detail(
    app: &GitronimoApp,
    workflow: &RepositoryWorkflow,
    git_actions: bool,
    colors: &ThemeColors,
    cx: &mut gpui::Context<GitronimoApp>,
) -> AnyElement {
    let head = app.head_branch_name();
    let current_topic = head
        .as_deref()
        .and_then(|branch| workflow.topic_for_branch(branch));
    let mut children = Vec::new();
    children.push(
        div()
            .text_lg()
            .font_weight(gpui::FontWeight::SEMIBOLD)
            .text_color(colors.text_primary)
            .child(workflow.kind.title())
            .into_any_element(),
    );
    children.push(
        div()
            .text_xs()
            .text_color(colors.text_muted)
            .child(workflow.kind.caption())
            .into_any_element(),
    );
    if git_actions {
        let mut actions = Vec::new();
        if current_topic.is_some() {
            actions.push(file_action_button(
                "Finish…",
                colors,
                cx,
                GitronimoApp::prompt_finish_workflow_topic,
            ));
            actions.push(file_action_button(
                "Sync",
                colors,
                cx,
                GitronimoApp::sync_workflow_head,
            ));
        }
        if !actions.is_empty() {
            children.push(div().flex().gap_1().children(actions).into_any_element());
        }
        if let Some(branch) = head.as_deref()
            && current_topic.is_none()
        {
            children.push(
                div()
                    .text_xs()
                    .text_color(colors.text_muted)
                    .child(format!(
                        "{branch} is not a topic branch. Start a type below, or check out feature/…"
                    ))
                    .into_any_element(),
            );
        }
    } else {
        children.push(
            div()
                .text_xs()
                .text_color(colors.text_muted)
                .child("Open the repository to start, finish, or sync topic branches.")
                .into_any_element(),
        );
    }
    children.push(section_label("Base branches", colors));
    for base in &workflow.bases {
        let parent = base
            .parent
            .as_deref()
            .map_or_else(|| "trunk".to_owned(), |parent| format!("parent {parent}"));
        let trunk = if base.is_trunk { " · trunk" } else { "" };
        children.push(kv_row(&base.name, &format!("{parent}{trunk}"), colors));
    }
    children.push(section_label("Topic types", colors));
    for (index, topic) in workflow.topics.iter().enumerate() {
        let prefix = topic.prefix.clone();
        let start = topic.start.clone();
        let into = topic.merge_into.join(", ");
        let strategy = topic.strategy.title();
        children.push(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .py_2()
                .border_b_1()
                .border_color(colors.separator)
                .child(
                    div()
                        .text_sm()
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .text_color(colors.text_primary)
                        .child(topic.label.clone()),
                )
                .child(div().text_xs().text_color(colors.text_muted).child(format!(
                    "prefix {prefix} · start {start} · finish into {into} · {strategy}"
                )))
                .child(
                    div()
                        .flex()
                        .gap_1()
                        .children(git_actions.then(|| {
                            topic_action_button(
                                SharedString::from(format!("workflow-start-{index}")),
                                "Start…",
                                colors,
                                cx,
                                move |app, cx| {
                                    app.prompt_start_workflow_topic(
                                        prefix.clone(),
                                        start.clone(),
                                        cx,
                                    );
                                },
                            )
                        }))
                        .child(topic_action_button(
                            SharedString::from(format!("workflow-strategy-{index}")),
                            "Cycle strategy",
                            colors,
                            cx,
                            move |app, cx| app.cycle_workflow_topic_strategy(index, cx),
                        )),
                )
                .into_any_element(),
        );
    }
    div()
        .flex()
        .flex_col()
        .gap_2()
        .p_4()
        .overflow_hidden()
        .children(children)
        .into_any_element()
}

fn section_label(title: &'static str, colors: &ThemeColors) -> AnyElement {
    div()
        .pt_2()
        .text_xs()
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .text_color(colors.text_muted)
        .child(title.to_ascii_uppercase())
        .into_any_element()
}

fn kv_row(title: &str, detail: &str, colors: &ThemeColors) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap_0p5()
        .py_1()
        .child(
            div()
                .text_sm()
                .text_color(colors.text_primary)
                .child(title.to_owned()),
        )
        .child(
            div()
                .text_xs()
                .text_color(colors.text_muted)
                .child(detail.to_owned()),
        )
        .into_any_element()
}

fn topic_action_button(
    id: SharedString,
    label: &'static str,
    colors: &ThemeColors,
    cx: &mut gpui::Context<GitronimoApp>,
    on_click: impl Fn(&mut GitronimoApp, &mut gpui::Context<GitronimoApp>) + 'static,
) -> AnyElement {
    div()
        .id(id)
        .h(px(28.0))
        .px_2()
        .flex()
        .items_center()
        .rounded(px(4.0))
        .bg(colors.raised_background)
        .cursor_pointer()
        .hover(|style| style.bg(colors.selection))
        .on_click(cx.listener(move |app, _: &ClickEvent, _, cx| on_click(app, cx)))
        .child(div().text_xs().text_color(colors.text_primary).child(label))
        .into_any_element()
}
