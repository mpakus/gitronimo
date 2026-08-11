//! Small, reusable render helpers shared across the desktop views.
//!
//! These are presentational only: they never touch Git or domain logic.

use git_domain::StatusEntry;
use gpui::{FocusHandle, IntoElement, KeyDownEvent, Render, Window, div, prelude::*, px, relative};
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

#[allow(clippy::redundant_closure_for_method_calls)]
pub(crate) fn primary_window_action_button(
    label: &'static str,
    enabled: bool,
    colors: &ThemeColors,
    cx: &mut gpui::Context<GitronimoApp>,
    on_click: impl Fn(&mut GitronimoApp, &mut Window, &mut gpui::Context<GitronimoApp>) + 'static,
) -> gpui::AnyElement {
    let tooltip_colors = *colors;
    let unavailable = if enabled {
        label
    } else {
        "Enter a subject and stage changes to commit"
    };
    div()
        .id(label)
        .h(px(26.0))
        .px_3()
        .flex()
        .items_center()
        .bg(if enabled {
            colors.accent
        } else {
            colors.raised_background
        })
        .rounded(px(4.0))
        .text_color(if enabled {
            colors.panel_background
        } else {
            colors.text_muted
        })
        .when(enabled, |button| button.cursor_pointer())
        .tooltip(move |_, cx| {
            cx.new(|_| ActionTooltip {
                label: unavailable,
                colors: tooltip_colors,
            })
            .into()
        })
        .when(enabled, |button| {
            button.on_click(cx.listener(move |app, _, window, cx| on_click(app, window, cx)))
        })
        .text_sm()
        .font_weight(gpui::FontWeight::MEDIUM)
        .child(label)
        .into_any_element()
}

pub(crate) fn commit_option_chip(
    label: &'static str,
    active: bool,
    colors: &ThemeColors,
    cx: &mut gpui::Context<GitronimoApp>,
    on_click: impl Fn(&mut GitronimoApp, &mut gpui::Context<GitronimoApp>) + 'static,
) -> gpui::AnyElement {
    div()
        .id(label)
        .h(px(22.0))
        .px_2()
        .flex()
        .items_center()
        .rounded(px(3.0))
        .bg(if active {
            colors.selection
        } else {
            colors.panel_background
        })
        .text_color(if active {
            colors.accent
        } else {
            colors.text_secondary
        })
        .cursor_pointer()
        .on_click(cx.listener(move |app, _, _, cx| on_click(app, cx)))
        .text_xs()
        .font_weight(gpui::FontWeight::MEDIUM)
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

pub(crate) fn status_badge_info(
    label: &str,
    colors: &ThemeColors,
) -> (&'static str, gpui::Rgba, gpui::Rgba) {
    let code = label.trim_start();
    let ch = code.chars().next().unwrap_or(' ');
    match ch {
        'M' => ("M", colors.accent, colors.accent),
        'A' => ("A", colors.success, colors.success),
        'D' => ("D", colors.danger, colors.danger),
        'U' => ("U", colors.warning, colors.warning),
        '?' => ("?", colors.text_muted, colors.text_muted),
        'R' => ("R", colors.accent, colors.accent),
        'C' => ("C", colors.warning, colors.warning),
        _ => (" ", colors.text_muted, colors.text_muted),
    }
}

pub(crate) fn status_badge_square(
    letter: &str,
    bg: gpui::Rgba,
    colors: &ThemeColors,
) -> gpui::AnyElement {
    div()
        .w(px(14.0))
        .h(px(14.0))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(2.0))
        .bg(bg)
        .text_xs()
        .font_weight(gpui::FontWeight::BOLD)
        .text_color(colors.panel_background)
        .child(letter.to_owned())
        .into_any_element()
}

pub(crate) fn sidebar_section_label(title: &'static str, colors: &ThemeColors) -> gpui::AnyElement {
    div()
        .px_3()
        .pt_3()
        .pb_1()
        .text_xs()
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .text_color(colors.text_muted)
        .child(title)
        .into_any_element()
}

pub(crate) fn centered_empty_state(
    title: &str,
    detail: &str,
    colors: &ThemeColors,
) -> gpui::AnyElement {
    div()
        .flex_1()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap_2()
        .p_6()
        .bg(colors.panel_background)
        .child(
            div()
                .text_2xl()
                .text_color(colors.text_muted)
                .child("\u{1F4C4}"),
        )
        .child(
            div()
                .text_sm()
                .font_weight(gpui::FontWeight::MEDIUM)
                .text_color(colors.text_secondary)
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

pub(crate) fn segmented_detail_toggle(
    left_label: &'static str,
    right_label: &'static str,
    left_active: bool,
    colors: &ThemeColors,
    cx: &mut gpui::Context<GitronimoApp>,
    on_left: impl Fn(&mut GitronimoApp, &mut gpui::Context<GitronimoApp>) + 'static,
    on_right: impl Fn(&mut GitronimoApp, &mut gpui::Context<GitronimoApp>) + 'static,
) -> gpui::AnyElement {
    div()
        .flex()
        .p_0p5()
        .bg(colors.raised_background)
        .rounded(px(4.0))
        .child(segmented_toggle_tab(
            left_label,
            left_active,
            colors,
            cx,
            on_left,
        ))
        .child(segmented_toggle_tab(
            right_label,
            !left_active,
            colors,
            cx,
            on_right,
        ))
        .into_any_element()
}

fn segmented_toggle_tab(
    label: &'static str,
    active: bool,
    colors: &ThemeColors,
    cx: &mut gpui::Context<GitronimoApp>,
    on_click: impl Fn(&mut GitronimoApp, &mut gpui::Context<GitronimoApp>) + 'static,
) -> gpui::AnyElement {
    let mut tab = div()
        .id(label)
        .px_2()
        .py_0p5()
        .rounded(px(3.0))
        .text_xs()
        .font_weight(gpui::FontWeight::MEDIUM)
        .bg(if active {
            colors.accent
        } else {
            colors.raised_background
        })
        .text_color(if active {
            colors.panel_background
        } else {
            colors.text_secondary
        })
        .cursor_pointer();
    tab.interactivity()
        .on_click(cx.listener(move |app, _, _, cx| on_click(app, cx)));
    tab.child(label).into_any_element()
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
    } else if activity.contains("complete")
        || activity.contains("refreshed")
        || activity.contains("opened")
    {
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

pub(crate) fn toolbar_search_field(
    placeholder: &'static str,
    value: &str,
    focus_handle: &FocusHandle,
    colors: &ThemeColors,
    cx: &mut gpui::Context<GitronimoApp>,
    on_change: impl Fn(&mut GitronimoApp, String, &mut gpui::Context<GitronimoApp>) + 'static,
) -> gpui::AnyElement {
    inline_search_field(
        placeholder,
        placeholder,
        value,
        focus_handle,
        colors,
        cx,
        on_change,
        true,
    )
}

#[allow(clippy::too_many_arguments, clippy::redundant_closure_for_method_calls)]
pub(crate) fn inline_search_field(
    field_id: &'static str,
    placeholder: &'static str,
    value: &str,
    focus_handle: &FocusHandle,
    colors: &ThemeColors,
    cx: &mut gpui::Context<GitronimoApp>,
    on_change: impl Fn(&mut GitronimoApp, String, &mut gpui::Context<GitronimoApp>) + 'static,
    show_label: bool,
) -> gpui::AnyElement {
    let display = if value.is_empty() {
        placeholder.to_owned()
    } else {
        value.to_owned()
    };
    let text_color = if value.is_empty() {
        colors.text_muted
    } else {
        colors.text_primary
    };
    let mut field = div()
        .id(field_id)
        .when(show_label, |element| element.w(px(168.0)))
        .when(!show_label, |element| element.flex_1())
        .h(px(24.0))
        .px_2()
        .flex()
        .items_center()
        .gap_1()
        .bg(colors.raised_background)
        .rounded_full()
        .border_1()
        .border_color(colors.border)
        .track_focus(focus_handle)
        .cursor_text()
        .on_key_down(cx.listener(move |app, event: &KeyDownEvent, window, cx| {
            handle_search_keydown(event, window, cx, &on_change, app);
        }));
    field = field.child(
        div()
            .text_xs()
            .text_color(colors.text_muted)
            .child("\u{2315}"),
    );
    field = field.child(
        div()
            .flex_1()
            .overflow_hidden()
            .whitespace_nowrap()
            .text_ellipsis()
            .text_xs()
            .text_color(text_color)
            .child(display),
    );
    if show_label {
        div()
            .ml_1()
            .flex()
            .flex_col()
            .items_center()
            .gap_0p5()
            .child(field)
            .child(
                div()
                    .text_xs()
                    .text_color(colors.text_muted)
                    .child("Search"),
            )
            .into_any_element()
    } else {
        field.into_any_element()
    }
}

fn handle_search_keydown(
    event: &KeyDownEvent,
    window: &mut Window,
    cx: &mut gpui::Context<GitronimoApp>,
    on_change: &impl Fn(&mut GitronimoApp, String, &mut gpui::Context<GitronimoApp>),
    app: &mut GitronimoApp,
) {
    let keystroke = &event.keystroke;
    if keystroke.modifiers.platform && keystroke.key == "v" {
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            let mut next = app.active_search_query().to_owned();
            next.push_str(&text.replace('\n', " "));
            on_change(app, next, cx);
        }
        return;
    }
    if keystroke.modifiers.platform || keystroke.modifiers.control {
        return;
    }
    let current = app.active_search_query().to_owned();
    match keystroke.key.as_str() {
        "backspace" => {
            let mut chars: Vec<char> = current.chars().collect();
            if chars.pop().is_some() {
                on_change(app, chars.into_iter().collect(), cx);
            }
        }
        "escape" | "enter" => {
            window.blur();
        }
        _ => {
            if let Some(ch) = keystroke.key_char.as_ref().filter(|ch| !ch.contains('\n')) {
                let mut next = current;
                next.push_str(ch);
                on_change(app, next, cx);
            }
        }
    }
}

pub(crate) fn welcome_rail_tab(
    label: &'static str,
    icon: &'static str,
    active: bool,
    colors: &ThemeColors,
    cx: &mut gpui::Context<GitronimoApp>,
    on_click: impl Fn(&mut GitronimoApp, &mut gpui::Context<GitronimoApp>) + 'static,
) -> gpui::AnyElement {
    div()
        .id(label)
        .w_full()
        .py_2()
        .flex()
        .flex_col()
        .items_center()
        .gap_0p5()
        .cursor_pointer()
        .bg(if active {
            colors.panel_background
        } else {
            colors.sidebar_background
        })
        .border_r_1()
        .border_color(if active {
            colors.accent
        } else {
            colors.sidebar_background
        })
        .text_color(if active {
            colors.text_primary
        } else {
            colors.text_muted
        })
        .on_click(cx.listener(move |app, _, _, cx| on_click(app, cx)))
        .child(div().text_sm().child(icon))
        .child(
            div()
                .text_xs()
                .font_weight(if active {
                    gpui::FontWeight::MEDIUM
                } else {
                    gpui::FontWeight::NORMAL
                })
                .child(label),
        )
        .into_any_element()
}

pub(crate) fn remote_progress_footer(
    label: &str,
    progress: f32,
    colors: &ThemeColors,
    cx: &mut gpui::Context<GitronimoApp>,
) -> gpui::AnyElement {
    let fill = progress.clamp(0.08, 0.92);
    div()
        .mt_auto()
        .mx_3()
        .mb_3()
        .p_2()
        .flex()
        .flex_col()
        .gap_1p5()
        .bg(colors.raised_background)
        .rounded(px(4.0))
        .border_1()
        .border_color(colors.border)
        .child(
            div()
                .text_xs()
                .text_color(colors.text_secondary)
                .child(label.to_owned()),
        )
        .child(
            div()
                .h(px(4.0))
                .w_full()
                .rounded_full()
                .bg(colors.panel_background)
                .overflow_hidden()
                .child(
                    div()
                        .h_full()
                        .w(relative(fill))
                        .bg(colors.accent)
                        .rounded_full(),
                ),
        )
        .child(crate::views::components::file_action_button(
            "Cancel",
            colors,
            cx,
            |app, cx| {
                app.cancel_network_operation(cx);
            },
        ))
        .into_any_element()
}

pub(crate) fn toolbar_divider(colors: &ThemeColors) -> gpui::AnyElement {
    div()
        .w(px(1.0))
        .h(px(32.0))
        .mx_1()
        .bg(colors.border)
        .into_any_element()
}

#[allow(clippy::redundant_closure)]
pub(crate) fn stacked_toolbar_button(
    tooltip_label: &'static str,
    icon: &'static str,
    colors: &ThemeColors,
    cx: &mut gpui::Context<GitronimoApp>,
    on_click: impl Fn(&mut GitronimoApp, &mut gpui::Window, &mut gpui::Context<GitronimoApp>) + 'static,
    disabled: bool,
) -> gpui::AnyElement {
    let tooltip_colors = *colors;
    let icon_color = if disabled {
        colors.text_muted
    } else {
        colors.text_primary
    };
    div()
        .id(tooltip_label)
        .w(px(44.0))
        .h(px(44.0))
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap_0p5()
        .rounded(px(4.0))
        .when(!disabled, gpui::Styled::cursor_pointer)
        .when(!disabled, |d| {
            d.tooltip(move |_, cx| {
                cx.new(|_| ActionTooltip {
                    label: tooltip_label,
                    colors: tooltip_colors,
                })
                .into()
            })
        })
        .when(!disabled, |d| {
            d.on_click(cx.listener(move |app, _, window, cx| {
                on_click(app, window, cx);
            }))
        })
        .child(div().text_sm().text_color(icon_color).child(icon))
        .child(
            div()
                .text_xs()
                .text_color(if disabled {
                    colors.text_muted
                } else {
                    colors.text_secondary
                })
                .child(tooltip_label),
        )
        .into_any_element()
}
