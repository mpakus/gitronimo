//! Small, reusable render helpers shared across the desktop views.
//!
//! These are presentational only: they never touch Git or domain logic.

use std::sync::{Arc, Mutex};

use git_domain::StatusEntry;
use gpui::{AnyElement, DragMoveEvent, IntoElement, Render, Window, div, prelude::*, px, relative};
use ui_kit::ThemeColors;

use crate::app_state::{GitronimoApp, Mutation, clamp_list_pane_width, clamp_sidebar_width};

/// Compact list row height (file lists, ref tree, welcome repos).
pub(crate) const LIST_ROW_HEIGHT: f32 = 22.0;
/// Sidebar navigation row height (Working Copy, History, etc.).
pub(crate) const NAV_ROW_HEIGHT: f32 = 24.0;
/// View panel header bar height (Stashes, Remotes, Services, PRs).
pub(crate) const PANEL_HEADER_HEIGHT: f32 = 28.0;
/// Primary/secondary action button height.
pub(crate) const ACTION_BUTTON_HEIGHT: f32 = 26.0;
/// Standard width for list panes in two-pane views.
pub(crate) const LIST_PANE_WIDTH: f32 = 280.0;
/// Hit target width for vertical pane resize handles.
pub(crate) const PANE_RESIZE_HIT_WIDTH: f32 = 8.0;

/// Distinct drag types so GPUI `on_drag_move` listeners do not cross-fire.
#[derive(Clone)]
struct SidebarResizeDrag {
    start_x: Arc<Mutex<f32>>,
    start_width: Arc<Mutex<f32>>,
}

#[derive(Clone)]
struct ListPaneResizeDrag {
    start_x: Arc<Mutex<f32>>,
    start_width: Arc<Mutex<f32>>,
}

impl Render for SidebarResizeDrag {
    fn render(&mut self, _: &mut Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
        div().w(px(0.0)).h(px(0.0))
    }
}

impl Render for ListPaneResizeDrag {
    fn render(&mut self, _: &mut Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
        div().w(px(0.0)).h(px(0.0))
    }
}

fn resize_handle_shell(id: &'static str, colors: &ThemeColors) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .w(px(PANE_RESIZE_HIT_WIDTH))
        .h_full()
        .flex_shrink_0()
        .flex()
        .justify_center()
        .cursor_col_resize()
        .hover(|style| style.bg(colors.selection))
        .child(div().w(px(2.0)).h_full().bg(colors.list_row_border))
}

/// Full-height handle between sidebar (panel 1) and content; only changes `sidebar_width`.
pub(crate) fn sidebar_resize_handle(
    start_width: f32,
    colors: &ThemeColors,
    cx: &mut gpui::Context<GitronimoApp>,
) -> AnyElement {
    let drag = SidebarResizeDrag {
        start_x: Arc::new(Mutex::new(0.0)),
        start_width: Arc::new(Mutex::new(start_width)),
    };
    resize_handle_shell("sidebar-resize-handle", colors)
        .on_drag(drag, move |drag, _offset, window, cx| {
            *drag.start_x.lock().unwrap() = f32::from(window.mouse_position().x);
            *drag.start_width.lock().unwrap() = start_width;
            cx.new(|_| drag.clone())
        })
        .on_drag_move(cx.listener(
            move |app, event: &DragMoveEvent<SidebarResizeDrag>, _, cx| {
                let drag = event.drag(cx);
                let start_x = *drag.start_x.lock().unwrap();
                let start_width = *drag.start_width.lock().unwrap();
                let delta = f32::from(event.event.position.x) - start_x;
                app.sidebar_width = clamp_sidebar_width(start_width + delta);
                let _ = app.store.save_sidebar_width(app.sidebar_width);
                cx.notify();
            },
        ))
        .into_any_element()
}

/// Full-height handle between list pane (panel 2) and detail (panel 3); only changes `column_width`.
pub(crate) fn list_pane_resize_handle(
    start_width: f32,
    colors: &ThemeColors,
    cx: &mut gpui::Context<GitronimoApp>,
) -> AnyElement {
    let drag = ListPaneResizeDrag {
        start_x: Arc::new(Mutex::new(0.0)),
        start_width: Arc::new(Mutex::new(start_width)),
    };
    resize_handle_shell("list-pane-resize-handle", colors)
        .on_drag(drag, move |drag, _offset, window, cx| {
            *drag.start_x.lock().unwrap() = f32::from(window.mouse_position().x);
            *drag.start_width.lock().unwrap() = start_width;
            cx.new(|_| drag.clone())
        })
        .on_drag_move(cx.listener(
            move |app, event: &DragMoveEvent<ListPaneResizeDrag>, _, cx| {
                let drag = event.drag(cx);
                let start_x = *drag.start_x.lock().unwrap();
                let start_width = *drag.start_width.lock().unwrap();
                let delta = f32::from(event.event.position.x) - start_x;
                app.column_width = clamp_list_pane_width(start_width + delta);
                let _ = app.store.save_list_pane_width(app.column_width);
                cx.notify();
            },
        ))
        .into_any_element()
}

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
        .h(px(ACTION_BUTTON_HEIGHT))
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
        .h(px(ACTION_BUTTON_HEIGHT))
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
        .h(px(ACTION_BUTTON_HEIGHT))
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
        .h(px(ACTION_BUTTON_HEIGHT))
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

#[allow(dead_code)]
pub(crate) fn commit_option_chip(
    label: &'static str,
    active: bool,
    colors: &ThemeColors,
    cx: &mut gpui::Context<GitronimoApp>,
    on_click: impl Fn(&mut GitronimoApp, &mut gpui::Context<GitronimoApp>) + 'static,
) -> gpui::AnyElement {
    div()
        .id(label)
        .h(px(LIST_ROW_HEIGHT))
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
        .h(px(ACTION_BUTTON_HEIGHT))
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

pub(crate) fn section_header(title: &'static str, colors: &ThemeColors) -> gpui::AnyElement {
    div()
        .px_3()
        .pt_2()
        .pb_1()
        .border_t_1()
        .border_color(colors.separator)
        .text_xs()
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .text_color(colors.text_muted)
        .child(title)
        .into_any_element()
}

/// First sidebar section label (no top divider).
pub(crate) fn sidebar_section_label_first(
    title: &'static str,
    colors: &ThemeColors,
) -> gpui::AnyElement {
    div()
        .px_3()
        .pt_2()
        .pb_1()
        .text_xs()
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .text_color(colors.text_muted)
        .child(title)
        .into_any_element()
}

pub(crate) fn sidebar_section_label(title: &'static str, colors: &ThemeColors) -> gpui::AnyElement {
    section_header(title, colors)
}

pub(crate) fn detail_section(title: &'static str, colors: &ThemeColors) -> gpui::AnyElement {
    div()
        .pt_4()
        .pb_1()
        .text_xs()
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .text_color(colors.text_muted)
        .child(title.to_uppercase())
        .into_any_element()
}

pub(crate) fn detail_row(label: &str, value: &str, colors: &ThemeColors) -> gpui::AnyElement {
    div()
        .py_1p5()
        .border_b_1()
        .border_color(colors.separator)
        .flex()
        .gap_4()
        .child(
            div()
                .w(px(140.0))
                .flex_shrink_0()
                .text_sm()
                .text_color(colors.text_muted)
                .child(label.to_owned()),
        )
        .child(
            div()
                .flex_1()
                .text_sm()
                .text_color(colors.text_primary)
                .child(value.to_owned()),
        )
        .into_any_element()
}

pub(crate) fn view_panel_header(
    title: &'static str,
    colors: &ThemeColors,
    actions: Option<gpui::AnyElement>,
) -> gpui::AnyElement {
    let mut header = div()
        .h(px(PANEL_HEADER_HEIGHT))
        .px_3()
        .flex()
        .items_center()
        .justify_between()
        .border_b_1()
        .border_color(colors.border)
        .child(
            div()
                .text_sm()
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(colors.text_primary)
                .child(title),
        );
    if let Some(actions) = actions {
        header = header.child(actions);
    }
    header.into_any_element()
}

pub(crate) fn two_pane_view(
    header: gpui::AnyElement,
    list: gpui::AnyElement,
    detail: gpui::AnyElement,
    colors: &ThemeColors,
) -> gpui::AnyElement {
    div()
        .flex()
        .flex_col()
        .h_full()
        .child(header)
        .child(
            div()
                .flex_1()
                .flex()
                .items_start()
                .child(
                    div()
                        .w(px(LIST_PANE_WIDTH))
                        .h_full()
                        .border_r_1()
                        .border_color(colors.border)
                        .overflow_hidden()
                        .child(list),
                )
                .child(div().flex_1().h_full().overflow_hidden().child(detail)),
        )
        .into_any_element()
}

pub(crate) fn head_badge(colors: &ThemeColors) -> gpui::AnyElement {
    div()
        .ml_auto()
        .flex_shrink_0()
        .px_1()
        .py_0p5()
        .rounded(px(3.0))
        .bg(colors.raised_background)
        .border_1()
        .border_color(colors.border)
        .text_xs()
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .text_color(colors.text_secondary)
        .child("HEAD")
        .into_any_element()
}

pub(crate) fn count_badge(text: String, inverted: bool, colors: &ThemeColors) -> gpui::AnyElement {
    div()
        .ml_auto()
        .min_w(px(18.0))
        .h(px(16.0))
        .px_1()
        .flex()
        .items_center()
        .justify_center()
        .rounded_full()
        .bg(if inverted {
            colors.panel_background
        } else {
            colors.raised_background
        })
        .text_xs()
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .text_color(if inverted {
            colors.accent
        } else {
            colors.text_primary
        })
        .child(text)
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

/// Calendar date as `M/D/YY` (UTC).
pub(crate) fn short_calendar_date(timestamp: i64) -> String {
    let (year, month, day) = civil_ymd(timestamp);
    let yy = year.rem_euclid(100);
    format!("{month}/{day}/{yy:02}")
}

/// Month group label such as `AUGUST 2026` (UTC).
pub(crate) fn month_group_label(timestamp: i64) -> String {
    const MONTHS: [&str; 12] = [
        "JANUARY",
        "FEBRUARY",
        "MARCH",
        "APRIL",
        "MAY",
        "JUNE",
        "JULY",
        "AUGUST",
        "SEPTEMBER",
        "OCTOBER",
        "NOVEMBER",
        "DECEMBER",
    ];
    let (year, month, _) = civil_ymd(timestamp);
    let name = MONTHS
        .get((month as usize).saturating_sub(1))
        .copied()
        .unwrap_or("UNKNOWN");
    format!("{name} {year}")
}

fn civil_ymd(timestamp: i64) -> (i32, u32, u32) {
    let days = timestamp.div_euclid(86_400);
    // Howard Hinnant civil_from_days (UTC).
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 }.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    let year = i32::try_from(y).unwrap_or(1970);
    let month = u32::try_from(m).unwrap_or(1).clamp(1, 12);
    let day = u32::try_from(d).unwrap_or(1).clamp(1, 31);
    (year, month, day)
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

pub(crate) fn stacked_toolbar_button(
    tooltip_label: &'static str,
    kind: crate::views::icons::IconKind,
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
        .w(px(48.0))
        .h(px(48.0))
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap_0p5()
        .rounded(px(6.0))
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
        .child(crate::views::icons::icon(kind, 16.0, icon_color))
        .child(
            div()
                .text_xs()
                .text_center()
                .text_color(if disabled {
                    colors.text_muted
                } else {
                    colors.text_secondary
                })
                .child(tooltip_label),
        )
        .into_any_element()
}
