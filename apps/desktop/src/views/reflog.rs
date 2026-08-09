//! Reflog view: a bounded list of HEAD reflog entries with a restore action
//! for recovering deleted branches.

use gpui::{ClickEvent, div, prelude::*, px};
use ui_kit::ThemeColors;

use git_domain::WorktreeRepository;

use crate::app_state::{GitronimoApp, RepositoryView};
use crate::views::components::file_action_button;

impl GitronimoApp {
    pub(crate) fn reflog_view(
        &self,
        _repository: &WorktreeRepository,
        colors: &ThemeColors,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let rows = self
            .reflog
            .iter()
            .enumerate()
            .map(|(index, entry)| {
                let when = format_timestamp(entry.identity.timestamp);
                (
                    index,
                    String::from_utf8_lossy(&entry.new_oid).to_string(),
                    String::from_utf8_lossy(&entry.identity.name).to_string(),
                    when,
                    entry.subject.clone(),
                )
            })
            .collect::<Vec<_>>();
        let selected = self.selected_reflog;
        let list_colors = *colors;
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(div().text_xl().child("Reflog"))
            .child(file_action_button("Working Copy", colors, cx, |app, cx| {
                app.navigate_to(RepositoryView::WorkingCopy, cx);
            }))
            .child(file_action_button(
                "Refresh reflog",
                colors,
                cx,
                |app, cx| {
                    app.reflog_load_token = app.reflog_load_token.wrapping_add(1);
                    if let crate::app_state::ShellState::Repository(repository) = &app.state {
                        app.load_reflog(repository.clone(), cx);
                    }
                },
            ))
            .child(file_action_button(
                "Restore branch from selected entry…",
                colors,
                cx,
                |_, cx| GitronimoApp::prompt_restore_branch_from_reflog(cx),
            ))
            .children(rows.into_iter().map(|(index, oid, name, when, subject)| {
                div()
                    .id(index)
                    .h(px(28.0))
                    .px_2()
                    .flex()
                    .items_center()
                    .bg(if selected == Some(index) {
                        list_colors.raised_background
                    } else {
                        list_colors.panel_background
                    })
                    .border_b_1()
                    .border_color(list_colors.border)
                    .cursor_pointer()
                    .on_click(cx.listener(move |app, _: &ClickEvent, _, cx| {
                        app.selected_reflog = Some(index);
                        cx.notify();
                    }))
                    .child(format!("{oid}  {name}  {when}  {subject}"))
            }))
            .child("Select an entry, then Restore to recreate a branch at that commit.".to_string())
    }
}

fn format_timestamp(timestamp: i64) -> String {
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
