//! Small, reusable render helpers shared across the desktop views.
//!
//! These are presentational only: they never touch Git or domain logic.

use git_domain::StatusEntry;
use gpui::{IntoElement, Render, Window, div, prelude::*, px};
use ui_kit::ThemeColors;

use crate::app_state::{GitronimoApp, Mutation};

#[derive(Default)]
pub(crate) struct StatusGroups<'a> {
    pub staged: Vec<&'a StatusEntry>,
    pub unstaged: Vec<&'a StatusEntry>,
    pub untracked: Vec<&'a StatusEntry>,
    pub conflicts: Vec<&'a StatusEntry>,
}

#[allow(dead_code)]
pub(crate) fn workspace_section(
    title: &'static str,
    content: impl IntoElement,
    colors: &ThemeColors,
) -> gpui::AnyElement {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(div().text_xs().text_color(colors.text_muted).child(title))
        .child(content)
        .into_any_element()
}

pub(crate) fn mutation_button(
    label: &'static str,
    disabled: bool,
    operation: Mutation,
    colors: &ThemeColors,
    cx: &mut gpui::Context<GitronimoApp>,
) -> gpui::AnyElement {
    let tooltip_colors = *colors;
    div()
        .id(label)
        .h(px(26.0))
        .px_2()
        .flex()
        .items_center()
        .bg(colors.raised_background)
        .rounded(px(4.0))
        .cursor_pointer()
        .tooltip(move |_, cx| {
            cx.new(|_| ActionTooltip {
                label,
                colors: tooltip_colors,
            })
            .into()
        })
        .on_click(cx.listener(move |app, _, _, cx| {
            if !disabled {
                app.mutate(operation, cx);
            }
        }))
        .text_sm()
        .child(label)
        .into_any_element()
}

pub(crate) fn file_action_button(
    label: &'static str,
    colors: &ThemeColors,
    cx: &mut gpui::Context<GitronimoApp>,
    on_click: impl Fn(&mut GitronimoApp, &mut gpui::Context<GitronimoApp>) + 'static,
) -> gpui::AnyElement {
    let tooltip_colors = *colors;
    div()
        .id(label)
        .h(px(26.0))
        .px_2()
        .flex()
        .items_center()
        .bg(colors.raised_background)
        .rounded(px(4.0))
        .cursor_pointer()
        .tooltip(move |_, cx| {
            cx.new(|_| ActionTooltip {
                label,
                colors: tooltip_colors,
            })
            .into()
        })
        .on_click(cx.listener(move |app, _, _, cx| on_click(app, cx)))
        .text_sm()
        .child(label)
        .into_any_element()
}

#[allow(dead_code)]
pub(crate) fn window_action_button(
    label: &'static str,
    colors: &ThemeColors,
    cx: &mut gpui::Context<GitronimoApp>,
    on_click: impl Fn(&mut GitronimoApp, &mut Window, &mut gpui::Context<GitronimoApp>) + 'static,
) -> gpui::AnyElement {
    let tooltip_colors = *colors;
    div()
        .id(label)
        .h(px(26.0))
        .px_2()
        .flex()
        .items_center()
        .bg(colors.raised_background)
        .rounded(px(4.0))
        .cursor_pointer()
        .tooltip(move |_, cx| {
            cx.new(|_| ActionTooltip {
                label,
                colors: tooltip_colors,
            })
            .into()
        })
        .on_click(cx.listener(move |app, _, window, cx| on_click(app, window, cx)))
        .text_sm()
        .child(label)
        .into_any_element()
}

pub(crate) fn primary_window_action_button(
    label: &'static str,
    colors: &ThemeColors,
    cx: &mut gpui::Context<GitronimoApp>,
    on_click: impl Fn(&mut GitronimoApp, &mut Window, &mut gpui::Context<GitronimoApp>) + 'static,
) -> gpui::AnyElement {
    let tooltip_colors = *colors;
    div()
        .id(label)
        .h(px(26.0))
        .px_2()
        .flex()
        .items_center()
        .bg(colors.accent)
        .rounded(px(4.0))
        .text_color(colors.panel_background)
        .cursor_pointer()
        .tooltip(move |_, cx| {
            cx.new(|_| ActionTooltip {
                label,
                colors: tooltip_colors,
            })
            .into()
        })
        .on_click(cx.listener(move |app, _, window, cx| on_click(app, window, cx)))
        .text_sm()
        .child(label)
        .into_any_element()
}

#[allow(dead_code)]
pub(crate) fn validated_action_button(
    label: &'static str,
    enabled: bool,
    unavailable_reason: &'static str,
    colors: &ThemeColors,
    cx: &mut gpui::Context<GitronimoApp>,
    on_click: impl Fn(&mut GitronimoApp, &mut gpui::Context<GitronimoApp>) + 'static,
) -> gpui::AnyElement {
    if enabled {
        return file_action_button(label, colors, cx, on_click);
    }
    let tooltip_colors = *colors;
    div()
        .id(label)
        .h(px(26.0))
        .px_2()
        .flex()
        .items_center()
        .bg(colors.raised_background)
        .rounded(px(4.0))
        .text_color(colors.text_muted)
        .tooltip(move |_, cx| {
            cx.new(|_| ActionTooltip {
                label: unavailable_reason,
                colors: tooltip_colors,
            })
            .into()
        })
        .text_sm()
        .child(label)
        .into_any_element()
}

pub(crate) fn status_path(entry: &StatusEntry) -> &git_domain::GitPath {
    match entry {
        StatusEntry::Ordinary { path, .. }
        | StatusEntry::Renamed { path, .. }
        | StatusEntry::Unmerged { path, .. }
        | StatusEntry::Untracked(path)
        | StatusEntry::Ignored(path) => path,
    }
}

pub(crate) fn status_label(entry: &StatusEntry) -> String {
    let path = String::from_utf8_lossy(&status_path(entry).0);
    match entry {
        StatusEntry::Ordinary { status, .. } => {
            format!("{}  {path}", String::from_utf8_lossy(&status.0))
        }
        StatusEntry::Renamed {
            status,
            source_path,
            ..
        } => format!(
            "{}  {} → {path}",
            String::from_utf8_lossy(&status.0),
            String::from_utf8_lossy(&source_path.0)
        ),
        StatusEntry::Unmerged { .. } => format!("UU  {path}"),
        StatusEntry::Untracked(_) => format!("??  {path}"),
        StatusEntry::Ignored(_) => format!("!!  {path}"),
    }
}

pub(crate) fn status_badge_info(label: &str) -> (&str, gpui::Rgba, gpui::Rgba) {
    let code = label.trim_start();
    let ch = code.chars().next().unwrap_or(' ');
    match ch {
        'M' => ("M", gpui::rgb(0x43_9a_ff), gpui::rgb(0xff_ff_ff)),
        'A' => ("A", gpui::rgb(0x12_8a_4b), gpui::rgb(0xff_ff_ff)),
        'D' => ("D", gpui::rgb(0xc7_28_3b), gpui::rgb(0xff_ff_ff)),
        'U' => ("U", gpui::rgb(0xa8_60_00), gpui::rgb(0xff_ff_ff)),
        '?' => ("?", gpui::rgb(0x7e_8c_9d), gpui::rgb(0xff_ff_ff)),
        'R' => ("R", gpui::rgb(0x7a_43_c8), gpui::rgb(0xff_ff_ff)),
        'C' => ("C", gpui::rgb(0xa8_60_00), gpui::rgb(0xff_ff_ff)),
        _ => (" ", gpui::rgb(0x7e_8c_9d), gpui::rgb(0xff_ff_ff)),
    }
}

pub(crate) fn loading_view(path: &std::path::Path, colors: &ThemeColors) -> impl IntoElement {
    state_panel(
        "Opening repository",
        &format!(
            "Checking {} with Git. This does not block the window.",
            path.display()
        ),
        colors.warning,
        colors,
    )
}

pub(crate) fn error_view(message: &str, colors: &ThemeColors) -> impl IntoElement {
    state_panel(
        "Unable to open repository",
        &format!("{message} Choose a different folder with Command-O."),
        colors.danger,
        colors,
    )
}

pub(crate) fn state_panel(
    title: &str,
    message: &str,
    accent: gpui::Rgba,
    colors: &ThemeColors,
) -> gpui::AnyElement {
    div()
        .p_4()
        .flex()
        .flex_col()
        .gap_2()
        .bg(colors.panel_background)
        .border_1()
        .border_color(colors.border)
        .child(div().text_color(accent).child(title.to_owned()))
        .child(
            div()
                .text_color(colors.text_secondary)
                .child(message.to_owned()),
        )
        .into_any_element()
}

#[allow(dead_code)]
pub(crate) fn empty_status_message(title: &str) -> &'static str {
    match title {
        "Staged" => "No staged files. Select a change, then stage it when it is ready to commit.",
        "Unstaged" => "No unstaged changes.",
        "Untracked" => "No untracked files.",
        "Conflicts" => "No merge conflicts.",
        _ => "Nothing here yet.",
    }
}

pub(crate) fn activity_color(activity: &str, colors: &ThemeColors) -> gpui::Rgba {
    if activity.contains("failed") || activity.contains("Unable") {
        colors.danger
    } else if activity.contains("complete") || activity.contains("refreshed") {
        colors.success
    } else if activity.ends_with('…') || activity.contains("in progress") {
        colors.warning
    } else {
        colors.text_secondary
    }
}

pub(crate) fn activity_label(activity: &str) -> String {
    if activity.ends_with('…') || activity.contains("in progress") {
        format!("● {activity}")
    } else {
        activity.to_owned()
    }
}

pub(crate) fn relative_time(timestamp: i64) -> String {
    let Ok(seconds) = u64::try_from(timestamp) else {
        return "unknown time".into();
    };
    let duration = std::time::Duration::from_secs(seconds);
    let Some(then) = std::time::UNIX_EPOCH.checked_add(duration) else {
        return "unknown time".into();
    };
    let Ok(age) = std::time::SystemTime::now().duration_since(then) else {
        return "unknown time".into();
    };
    let seconds = age.as_secs();
    if seconds < 60 {
        format!("{seconds}s ago")
    } else if seconds < 3600 {
        format!("{}m ago", seconds / 60)
    } else if seconds < 86_400 {
        format!("{}h ago", seconds / 3600)
    } else {
        format!("{}d ago", seconds / 86_400)
    }
}

pub(crate) struct ActionTooltip {
    pub label: &'static str,
    pub colors: ThemeColors,
}

impl Render for ActionTooltip {
    fn render(&mut self, _: &mut Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
        div()
            .px_2()
            .py_1()
            .bg(self.colors.raised_background)
            .border_1()
            .border_color(self.colors.border)
            .text_color(self.colors.text_primary)
            .child(self.label)
    }
}
