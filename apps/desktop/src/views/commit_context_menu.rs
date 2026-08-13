//! Tower-style right-click menu for History commits.
//!
//! Grouping and quoted OID labels follow Tower's Commit History menus
//! (approach-only). Chrome matches the sidebar ref context menu.

use gpui::{AnyElement, ClickEvent, MouseButton, MouseDownEvent, div, prelude::*, px};
use ui_kit::ThemeColors;

use crate::app_state::GitronimoApp;

impl GitronimoApp {
    #[allow(clippy::too_many_lines)]
    pub(crate) fn commit_context_menu_view(
        &self,
        colors: &ThemeColors,
        cx: &mut gpui::Context<Self>,
    ) -> Option<AnyElement> {
        let context = self.commit_context.clone()?;
        let quoted = format!("\"{}\"", context.short_oid);
        let head_scope = self.history_is_head_branch_scope();
        let scope_reason = "Available only when History shows the current HEAD branch.";
        let head_only_reason = "Available only for the current HEAD commit.";
        let mut menu = menu_shell(colors);

        let oid = context.oid.clone();
        menu = menu.child(owned_menu_item(
            "Copy Commit Hash".into(),
            true,
            None,
            colors,
            cx,
            move |app, _, cx| app.copy_commit_hash(&oid, cx),
        ));
        let oid = context.oid.clone();
        let index = context.index;
        menu = menu.child(owned_menu_item(
            "Copy Commit Info".into(),
            true,
            None,
            colors,
            cx,
            move |app, _, cx| app.copy_commit_info(index, &oid, cx),
        ));
        let index = context.index;
        menu = menu.child(owned_menu_item(
            "Reveal in History".into(),
            true,
            None,
            colors,
            cx,
            move |app, _, cx| app.reveal_history_commit(index, cx),
        ));

        menu = menu.child(menu_separator(colors));

        let oid = context.oid.clone();
        menu = menu.child(owned_menu_item(
            format!("Check Out {quoted}"),
            true,
            None,
            colors,
            cx,
            move |app, _, cx| app.checkout_detached_commit(&oid, cx),
        ));

        menu = menu.child(menu_separator(colors));

        let oid = context.oid.clone();
        menu = menu.child(owned_menu_item(
            format!("Reset HEAD to {quoted}…"),
            head_scope,
            (!head_scope).then_some(scope_reason),
            colors,
            cx,
            move |app, _, cx| app.prompt_reset_head_to(&oid, cx),
        ));
        let oid = context.oid.clone();
        menu = menu.child(owned_menu_item(
            format!("Revert {quoted}…"),
            head_scope,
            (!head_scope).then_some(scope_reason),
            colors,
            cx,
            move |app, _, cx| app.prompt_revert_commit(&oid, cx),
        ));
        let oid = context.oid.clone();
        menu = menu.child(owned_menu_item(
            format!("Rebase {quoted} Onto…"),
            head_scope,
            (!head_scope).then_some(scope_reason),
            colors,
            cx,
            move |app, _, cx| app.prompt_rebase_onto_commit(&oid, cx),
        ));

        menu = menu.child(menu_separator(colors));

        menu = menu.child(owned_menu_item(
            format!("Amend {quoted}"),
            context.is_head,
            (!context.is_head).then_some(head_only_reason),
            colors,
            cx,
            move |app, _, cx| {
                app.navigate_to(crate::app_state::RepositoryView::WorkingCopy, cx);
                if !app.commit_amend {
                    app.toggle_commit_amend(cx);
                }
            },
        ));
        menu = menu.child(owned_menu_item(
            format!("Edit {quoted}"),
            false,
            Some("Interactive rebase edit is not offered from History yet."),
            colors,
            cx,
            move |_, _, _| {},
        ));
        menu = menu.child(owned_menu_item(
            "Edit Commit Message…".into(),
            context.is_head,
            (!context.is_head).then_some(head_only_reason),
            colors,
            cx,
            move |app, _, cx| app.prompt_reword_last_commit(cx),
        ));
        let oid = context.oid.clone();
        menu = menu.child(owned_menu_item(
            format!("Delete {quoted}"),
            head_scope,
            (!head_scope).then_some(scope_reason),
            colors,
            cx,
            move |app, _, cx| app.prompt_drop_history_commit(&oid, cx),
        ));

        menu = menu.child(menu_separator(colors));

        let oid = context.oid.clone();
        menu = menu.child(owned_menu_item(
            "Create New Branch from…".into(),
            true,
            None,
            colors,
            cx,
            move |app, _, cx| app.prompt_create_branch_from_ref(oid.clone(), cx),
        ));
        let oid = context.oid.clone();
        menu = menu.child(owned_menu_item(
            "Create New Tag from…".into(),
            true,
            None,
            colors,
            cx,
            move |app, _, cx| app.prompt_create_tag_from_ref(oid.clone(), cx),
        ));

        menu = menu.child(menu_separator(colors));

        let oid = context.oid.clone();
        menu = menu.child(owned_menu_item(
            "Save Patch…".into(),
            true,
            None,
            colors,
            cx,
            move |app, _, cx| app.save_commit_patch(&oid, cx),
        ));
        let oid = context.oid.clone();
        menu = menu.child(owned_menu_item(
            "Export Files…".into(),
            true,
            None,
            colors,
            cx,
            move |app, _, cx| app.export_branch_archive(&oid, cx),
        ));
        let oid = context.oid.clone();
        menu = menu.child(owned_menu_item(
            format!("Compare {quoted} to Revision…"),
            true,
            None,
            colors,
            cx,
            move |app, _, cx| {
                app.begin_text_prompt(
                    crate::app_state::TextPromptKind::CompareTo { left: oid.clone() },
                    "",
                    cx,
                );
            },
        ));

        Some(menu.into_any_element())
    }

    pub(crate) fn commit_context_menu_overlay(
        &self,
        colors: &ThemeColors,
        cx: &mut gpui::Context<Self>,
    ) -> AnyElement {
        let (x, y) = self.commit_context_menu_position.unwrap_or((12.0, 90.0));
        div()
            .absolute()
            .inset_0()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|app, _: &MouseDownEvent, _, cx| {
                    app.close_commit_context_menu(cx);
                }),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|app, _: &MouseDownEvent, _, cx| {
                    app.close_commit_context_menu(cx);
                }),
            )
            .child(
                gpui::anchored()
                    .position(gpui::point(px(x), px(y)))
                    .anchor(gpui::Corner::TopLeft)
                    .snap_to_window_with_margin(gpui::Edges::all(px(8.0)))
                    .child(
                        div()
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|_, _: &MouseDownEvent, _, cx| {
                                    cx.stop_propagation();
                                }),
                            )
                            .on_mouse_down(
                                MouseButton::Right,
                                cx.listener(|_, _: &MouseDownEvent, _, cx| {
                                    cx.stop_propagation();
                                }),
                            )
                            .children(self.commit_context_menu_view(colors, cx)),
                    ),
            )
            .into_any_element()
    }
}

fn menu_shell(colors: &ThemeColors) -> gpui::Stateful<gpui::Div> {
    div()
        .id("commit-context-menu")
        .p_1p5()
        .flex()
        .flex_col()
        .gap_0p5()
        .min_w(px(280.0))
        .max_w(px(420.0))
        .bg(colors.panel_background)
        .border_1()
        .border_color(colors.border)
        .rounded(px(8.0))
        .shadow_lg()
}

fn menu_separator(colors: &ThemeColors) -> AnyElement {
    div().h(px(1.0)).my_1().bg(colors.border).into_any_element()
}

fn owned_menu_item(
    label: String,
    enabled: bool,
    disabled_reason: Option<&'static str>,
    colors: &ThemeColors,
    cx: &mut gpui::Context<GitronimoApp>,
    on_click: impl Fn(&mut GitronimoApp, &ClickEvent, &mut gpui::Context<GitronimoApp>) + 'static,
) -> AnyElement {
    let tooltip_colors = *colors;
    let reason = disabled_reason.unwrap_or("Unavailable");
    let mut item = div()
        .id(gpui::ElementId::Name(
            format!("commit-context-menu-item:{label}").into(),
        ))
        .h(px(24.0))
        .px_2()
        .flex()
        .items_center()
        .text_sm()
        .rounded(px(3.0))
        .text_color(if enabled {
            colors.text_primary
        } else {
            colors.text_muted
        });
    if enabled {
        item = item
            .cursor_pointer()
            .hover(|style| style.bg(colors.selection));
        item.interactivity()
            .on_click(cx.listener(move |app, event, _, cx| {
                app.close_commit_context_menu(cx);
                on_click(app, event, cx);
            }));
    } else {
        item = item.tooltip(move |_, cx| {
            cx.new(|_| crate::views::components::ActionTooltip {
                label: reason,
                colors: tooltip_colors,
            })
            .into()
        });
    }
    item.child(label).into_any_element()
}
