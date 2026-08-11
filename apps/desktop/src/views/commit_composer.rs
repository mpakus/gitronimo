//! Commit composer: Tower-like expandable card (subject → details on focus).

use std::time::Duration;

use gpui::{Animation, AnimationExt, AnyElement, div, ease_in_out, prelude::*, px};
use ui_kit::ThemeColors;

use git_domain::HeadStatus;

use crate::app_state::{GitronimoApp, Mutation};
use crate::views::components::{mutation_button, primary_window_action_button};
use crate::views::icons::{IconKind, icon};
use crate::views::single_line_input::{composer_multiline_shell, composer_subject_shell};

impl GitronimoApp {
    #[allow(clippy::too_many_lines)]
    pub(crate) fn commit_composer_view(
        &self,
        colors: &ThemeColors,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let staged_count = self.status_groups().staged.len();
        let amending = self.commit_amend;
        let enabled = !self.mutation_in_flight
            && !self.commit_subject.trim().is_empty()
            && (staged_count > 0 || amending);
        let primary_label = if amending { "Amend" } else { "Commit" };
        let subject_remaining = 50usize.saturating_sub(self.commit_subject.chars().count());
        let groups = self.status_groups();
        let has_stageable = !groups.unstaged.is_empty()
            || !groups.untracked.is_empty()
            || !groups.conflicts.is_empty();
        let (branch, tracking) = self.branch_path_labels();
        let subject_filled = !self.commit_subject.trim().is_empty();
        let expanded = self.commit_composer_expanded;

        // Card padding (`p_3`) is the only horizontal inset for fields and footer.
        let mut card = div()
            .w_full()
            .flex_shrink_0()
            .m_2()
            .px_3()
            .pt_3()
            .pb_3()
            .flex()
            .flex_col()
            .gap_2()
            .rounded(px(8.0))
            .bg(colors.raised_background)
            .border_1()
            .border_color(colors.border)
            .child(
                div()
                    .w_full()
                    .flex()
                    .items_center()
                    .gap_1p5()
                    .child(icon(IconKind::Branch, 14.0, colors.accent))
                    .child(
                        div()
                            .text_sm()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(colors.text_primary)
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .child(branch),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(colors.text_muted)
                            .child("\u{203A}"),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.0))
                            .text_xs()
                            .text_color(colors.text_secondary)
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .child(tracking),
                    ),
            )
            .child(composer_subject_shell(
                self.commit_subject_input.clone(),
                colors,
                subject_filled || expanded,
                subject_remaining.to_string(),
            ));

        if expanded {
            let (author_name, author_email) = split_author_identity(&self.author_identity);
            let initial = author_name
                .chars()
                .next()
                .map_or_else(|| "?".into(), |c| c.to_uppercase().to_string());

            // Description must be a direct full-width flex_col child (like subject).
            // Wrapping it in with_animation collapsed w_full against an indefinite
            // containing block → ~0px vertical slit. Do not touch subject mounting.
            card = card
                .child(composer_multiline_shell(
                    self.commit_body_input.clone(),
                    colors,
                    self.commit_body_focused,
                ))
                .child(
                    div()
                        .id("commit-composer-options")
                        .w_full()
                        .flex_shrink_0()
                        .flex()
                        .items_center()
                        .gap_3()
                        .child(commit_checkbox(
                            "Amend",
                            self.commit_amend,
                            colors,
                            cx,
                            GitronimoApp::toggle_commit_amend,
                        ))
                        .children(self.commit_amend_short_oid.as_ref().map(|oid| {
                            div()
                                .text_xs()
                                .font_family("Monaco")
                                .text_color(colors.text_muted)
                                .child(oid.clone())
                                .into_any_element()
                        }))
                        .child(commit_checkbox(
                            "Sign-off",
                            self.commit_sign_off,
                            colors,
                            cx,
                            GitronimoApp::toggle_commit_sign_off,
                        ))
                        .child(div().flex_1())
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_2()
                                .child(
                                    div()
                                        .w(px(22.0))
                                        .h(px(22.0))
                                        .rounded_full()
                                        .bg(colors.selection)
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .text_xs()
                                        .font_weight(gpui::FontWeight::SEMIBOLD)
                                        .text_color(colors.text_primary)
                                        .child(initial),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .min_w(px(0.0))
                                        .overflow_hidden()
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_weight(gpui::FontWeight::MEDIUM)
                                                .text_color(colors.text_primary)
                                                .overflow_hidden()
                                                .whitespace_nowrap()
                                                .child(author_name),
                                        )
                                        .children((!author_email.is_empty()).then(|| {
                                            div()
                                                .text_xs()
                                                .text_color(colors.text_muted)
                                                .overflow_hidden()
                                                .whitespace_nowrap()
                                                .child(author_email)
                                                .into_any_element()
                                        })),
                                ),
                        )
                        .with_animation(
                            "commit-options-fade",
                            Animation::new(Duration::from_millis(140)).with_easing(ease_in_out),
                            gpui::Styled::opacity,
                        ),
                );
        }

        card.child(
            div()
                .w_full()
                .flex()
                .items_center()
                .gap_2()
                .pt_1()
                .child(mutation_button(
                    "Stage All",
                    self.mutation_in_flight || !has_stageable,
                    Mutation::StageAll,
                    colors,
                    cx,
                ))
                .child(
                    div()
                        .flex_1()
                        .text_xs()
                        .text_color(colors.text_muted)
                        .child(if staged_count > 0 {
                            format!("{staged_count} staged")
                        } else {
                            String::new()
                        }),
                )
                .child(primary_window_action_button(
                    primary_label,
                    enabled,
                    colors,
                    cx,
                    move |app, _, cx| {
                        app.commit_draft(cx);
                    },
                )),
        )
    }

    fn branch_path_labels(&self) -> (String, String) {
        self.working_copy.as_ref().map_or_else(
            || ("Branch loading…".to_owned(), String::new()),
            |status| {
                let branch = match &status.branch.head {
                    HeadStatus::Branch(name) => String::from_utf8_lossy(&name.0).into_owned(),
                    HeadStatus::Detached => "Detached HEAD".into(),
                    HeadStatus::Unborn => "Unborn branch".into(),
                    HeadStatus::Unknown => "Unknown branch".into(),
                };
                let tracking = status.branch.upstream.as_ref().map_or_else(
                    || "No Tracking".to_owned(),
                    |upstream| String::from_utf8_lossy(&upstream.0).into_owned(),
                );
                (branch, tracking)
            },
        )
    }
}

fn split_author_identity(identity: &str) -> (String, String) {
    let identity = identity.trim();
    if identity.is_empty() || identity.starts_with("Loading") {
        return ("Unknown".into(), String::new());
    }
    if let Some((name, rest)) = identity.rsplit_once('<') {
        let email = rest.trim().trim_end_matches('>').trim().to_owned();
        let name = name.trim().to_owned();
        if name.is_empty() {
            (email.clone(), email)
        } else {
            (name, email)
        }
    } else {
        (identity.to_owned(), String::new())
    }
}

fn commit_checkbox(
    label: &'static str,
    checked: bool,
    colors: &ThemeColors,
    cx: &mut gpui::Context<GitronimoApp>,
    on_click: impl Fn(&mut GitronimoApp, &mut gpui::Context<GitronimoApp>) + 'static,
) -> AnyElement {
    div()
        .id(label)
        .flex()
        .items_center()
        .gap_1p5()
        .cursor_pointer()
        .on_click(cx.listener(move |app, _, _, cx| on_click(app, cx)))
        .child(
            div()
                .w(px(14.0))
                .h(px(14.0))
                .rounded(px(3.0))
                .border_1()
                .border_color(if checked {
                    colors.accent
                } else {
                    colors.border
                })
                .bg(if checked {
                    colors.accent
                } else {
                    colors.panel_background
                })
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .text_xs()
                        .font_weight(gpui::FontWeight::BOLD)
                        .text_color(colors.panel_background)
                        .child(if checked { "\u{2713}" } else { "" }),
                ),
        )
        .child(
            div()
                .text_xs()
                .text_color(if checked {
                    colors.text_primary
                } else {
                    colors.text_secondary
                })
                .child(label),
        )
        .into_any_element()
}
