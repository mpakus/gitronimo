//! Small, reusable render helpers shared across the desktop views.
//!
//! These are presentational only: they never touch Git or domain logic.

use git_domain::StatusEntry;
use gpui::{IntoElement, Render, Window, div, prelude::*};
use ui_kit::ThemeColors;

use crate::app_state::{GitronimoApp, Mutation};

#[derive(Default)]
pub(crate) struct StatusGroups<'a> {
    pub staged: Vec<&'a StatusEntry>,
    pub unstaged: Vec<&'a StatusEntry>,
    pub untracked: Vec<&'a StatusEntry>,
    pub conflicts: Vec<&'a StatusEntry>,
}

pub(crate) fn workspace_section(
    title: &'static str,
    content: impl IntoElement,
    colors: &ThemeColors,
) -> gpui::AnyElement {
    div()
        .p_3()
        .flex()
        .flex_col()
        .gap_2()
        .bg(colors.panel_background)
        .border_1()
        .border_color(colors.border)
        .child(
            div()
                .text_sm()
                .text_color(colors.text_secondary)
                .child(title),
        )
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
        .px_2()
        .py_1()
        .bg(colors.raised_background)
        .border_1()
        .border_color(colors.border)
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
        .px_2()
        .py_1()
        .bg(colors.panel_background)
        .border_1()
        .border_color(colors.border)
        .cursor_pointer()
        .tooltip(move |_, cx| {
            cx.new(|_| ActionTooltip {
                label,
                colors: tooltip_colors,
            })
            .into()
        })
        .on_click(cx.listener(move |app, _, _, cx| on_click(app, cx)))
        .child(label)
        .into_any_element()
}

pub(crate) fn window_action_button(
    label: &'static str,
    colors: &ThemeColors,
    cx: &mut gpui::Context<GitronimoApp>,
    on_click: impl Fn(&mut GitronimoApp, &mut Window, &mut gpui::Context<GitronimoApp>) + 'static,
) -> gpui::AnyElement {
    let tooltip_colors = *colors;
    div()
        .id(label)
        .px_3()
        .py_2()
        .bg(colors.raised_background)
        .border_1()
        .border_color(colors.border)
        .cursor_pointer()
        .tooltip(move |_, cx| {
            cx.new(|_| ActionTooltip {
                label,
                colors: tooltip_colors,
            })
            .into()
        })
        .on_click(cx.listener(move |app, _, window, cx| on_click(app, window, cx)))
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
        .px_3()
        .py_2()
        .bg(colors.accent)
        .border_1()
        .border_color(colors.accent)
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
        .child(label)
        .into_any_element()
}

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
        .px_2()
        .py_1()
        .bg(colors.raised_background)
        .border_1()
        .border_color(colors.border)
        .text_color(colors.text_muted)
        .tooltip(move |_, cx| {
            cx.new(|_| ActionTooltip {
                label: unavailable_reason,
                colors: tooltip_colors,
            })
            .into()
        })
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
