//! Right-click menu for sidebar branches, tags, and remotes.
//!
//! Labels quote the ref name (`Rename "main"…`). Items that
//! cannot run are disabled and carry a reason. Submenus (`Push To ▸`,
//! `Track Upstream Branch ▸`) open as a flyout beside the parent item.

use gpui::{AnyElement, ClickEvent, MouseButton, MouseDownEvent, div, prelude::*, px};
use ui_kit::ThemeColors;

use crate::app_state::{GitronimoApp, RefContext, RefContextSubmenu, TextPromptKind};

impl GitronimoApp {
    #[allow(clippy::too_many_lines)]
    pub(crate) fn ref_context_menu_view(
        &self,
        colors: &ThemeColors,
        cx: &mut gpui::Context<Self>,
    ) -> Option<AnyElement> {
        let context = self.ref_context.clone()?;
        let mut menu = menu_shell(colors);
        match context {
            RefContext::LocalBranch(branch) => {
                menu = self.extend_local_branch_menu(menu, &branch, colors, cx);
            }
            RefContext::RemoteBranch(branch) => {
                menu = Self::extend_remote_branch_menu(menu, &branch, colors, cx);
            }
            RefContext::Tag(tag) => {
                menu = Self::extend_tag_menu(menu, &tag, colors, cx);
            }
            RefContext::Remote(remote) => {
                menu = Self::extend_remote_menu(menu, &remote, colors, cx);
            }
        }
        Some(menu.into_any_element())
    }

    #[allow(clippy::too_many_lines)]
    fn extend_local_branch_menu(
        &self,
        mut menu: gpui::Stateful<gpui::Div>,
        branch: &str,
        colors: &ThemeColors,
        cx: &mut gpui::Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let head = self.head_branch_name();
        let is_head = head.as_deref() == Some(branch);
        let current = head.clone().unwrap_or_else(|| "HEAD".into());
        let named = NamedRefLabel::new(branch);
        let pinned = self
            .branch_organization
            .pinned
            .iter()
            .any(|name| name == branch);
        let archived = self
            .branch_organization
            .archived
            .iter()
            .any(|name| name == branch);
        let upstream = self.local_branch_upstream(branch);
        let has_upstream = upstream.is_some();

        let pin_label = if pinned {
            format!("Unpin {}", named.quoted())
        } else {
            format!("Pin {}", named.quoted())
        };
        let pin_branch = branch.to_owned();
        menu = menu.child(owned_menu_item(
            pin_label,
            true,
            None,
            colors,
            cx,
            move |app, _, cx| app.toggle_branch_pin(&pin_branch, cx),
        ));

        menu = menu.child(menu_separator(colors));

        let pull_ok = is_head;
        menu = menu.child(owned_menu_item(
            "Pull…".into(),
            pull_ok,
            (!pull_ok).then_some("Checkout this branch before pulling."),
            colors,
            cx,
            move |app, _, cx| app.pull_current(cx),
        ));
        let push_branch = branch.to_owned();
        menu = menu.child(owned_menu_item(
            "Push…".into(),
            true,
            None,
            colors,
            cx,
            move |app, _, cx| app.push_branch(&push_branch, cx),
        ));
        menu = menu.child(owned_menu_item(
            "Force Push with Lease…".into(),
            is_head,
            (!is_head).then_some("Force push applies to the checked-out branch."),
            colors,
            cx,
            move |app, _, cx| app.request_force_with_lease(cx),
        ));
        menu = menu.child(owned_menu_item(
            "Sync…".into(),
            is_head,
            (!is_head).then_some("Sync applies to the checked-out branch."),
            colors,
            cx,
            move |app, _, cx| app.sync_current(cx),
        ));

        menu = menu.child(menu_separator(colors));

        let publish_branch = branch.to_owned();
        menu = menu.child(owned_menu_item(
            format!("Publish {}…", named.quoted()),
            !has_upstream,
            has_upstream.then_some("This branch already tracks a remote."),
            colors,
            cx,
            move |app, _, cx| app.publish_branch(&publish_branch, cx),
        ));
        menu = menu.child(submenu_item(
            format!("Push {} To", named.quoted()),
            RefContextSubmenu::PushTo,
            self.ref_context_submenu == Some(RefContextSubmenu::PushTo),
            colors,
            cx,
        ));

        menu = menu.child(menu_separator(colors));

        if has_upstream {
            let stop = branch.to_owned();
            let upstream_label = upstream.clone().unwrap_or_default();
            menu = menu.child(owned_menu_item(
                format!("Stop Tracking {upstream_label}"),
                true,
                None,
                colors,
                cx,
                move |app, _, cx| app.unset_branch_upstream(&stop, cx),
            ));
        }
        menu = menu.child(submenu_item(
            "Track Upstream Branch".into(),
            RefContextSubmenu::TrackUpstream,
            self.ref_context_submenu == Some(RefContextSubmenu::TrackUpstream),
            colors,
            cx,
        ));

        menu = menu.child(menu_separator(colors));

        let merge_branch = branch.to_owned();
        let merge_label = format!("Merge {} into {}", named.quoted(), quote(&current));
        menu = menu.child(owned_menu_item(
            merge_label,
            !is_head,
            is_head.then_some("Cannot merge a branch into itself."),
            colors,
            cx,
            move |app, _, cx| app.merge_branch_into_current(merge_branch.clone(), cx),
        ));
        let rebase_branch = branch.to_owned();
        let rebase_label = format!("Rebase {} onto {}", quote(&current), named.quoted());
        menu = menu.child(owned_menu_item(
            rebase_label,
            !is_head,
            is_head.then_some("Cannot rebase a branch onto itself."),
            colors,
            cx,
            move |app, _, cx| app.rebase_current_onto(rebase_branch.clone(), cx),
        ));
        menu = menu.child(owned_menu_item(
            "Merge With Revision…".into(),
            true,
            None,
            colors,
            cx,
            move |app, _, cx| {
                app.begin_text_prompt(TextPromptKind::MergeRevision, "", cx);
            },
        ));
        menu = menu.child(owned_menu_item(
            "Rebase Onto Revision…".into(),
            true,
            None,
            colors,
            cx,
            move |app, _, cx| {
                app.begin_text_prompt(TextPromptKind::RebaseOnto, "", cx);
            },
        ));

        menu = menu.child(menu_separator(colors));

        let archive_branch = branch.to_owned();
        let archive_label = if archived {
            format!("Unarchive {}", named.quoted())
        } else {
            format!("Archive {}", named.quoted())
        };
        menu = menu.child(owned_menu_item(
            archive_label,
            true,
            None,
            colors,
            cx,
            move |app, _, cx| app.toggle_branch_archive(&archive_branch, cx),
        ));

        menu = menu.child(menu_separator(colors));

        let rename_branch = branch.to_owned();
        menu = menu.child(owned_menu_item(
            format!("Rename {}…", named.quoted()),
            true,
            None,
            colors,
            cx,
            move |app, _, cx| app.prompt_rename_branch(rename_branch.clone(), cx),
        ));
        let delete_branch = branch.to_owned();
        menu = menu.child(owned_menu_item(
            format!("Delete {}…", named.quoted()),
            !is_head,
            is_head.then_some("Checkout another branch before deleting HEAD."),
            colors,
            cx,
            move |app, _, cx| app.request_branch_delete(&delete_branch, cx),
        ));

        menu = menu.child(menu_separator(colors));

        let create_start = branch.to_owned();
        menu = menu.child(owned_menu_item(
            format!("Create New Branch from {}…", named.quoted()),
            true,
            None,
            colors,
            cx,
            move |app, _, cx| app.prompt_create_branch_from_ref(create_start.clone(), cx),
        ));
        let tag_start = branch.to_owned();
        menu = menu.child(owned_menu_item(
            format!("Create New Tag from {}…", named.quoted()),
            true,
            None,
            colors,
            cx,
            move |app, _, cx| app.prompt_create_tag_from_ref(tag_start.clone(), cx),
        ));
        let pr_head = branch.to_owned();
        menu = menu.child(owned_menu_item(
            format!("Create New Pull Request from {}…", named.quoted()),
            self.can_create_pull_request(),
            (!self.can_create_pull_request())
                .then_some("Connect a GitHub account in Settings first."),
            colors,
            cx,
            move |app, _, cx| app.prompt_create_pull_request_from_branch(&pr_head, cx),
        ));

        menu = menu.child(menu_separator(colors));

        let export_ref = branch.to_owned();
        menu = menu.child(owned_menu_item(
            format!("Export Files from {}…", named.quoted()),
            true,
            None,
            colors,
            cx,
            move |app, _, cx| app.export_branch_archive(&export_ref, cx),
        ));
        let compare_left = branch.to_owned();
        menu = menu.child(owned_menu_item(
            format!("Compare {} with Revision…", named.quoted()),
            true,
            None,
            colors,
            cx,
            move |app, _, cx| {
                app.begin_text_prompt(
                    TextPromptKind::CompareTo {
                        left: compare_left.clone(),
                    },
                    "",
                    cx,
                );
            },
        ));

        menu = menu.child(menu_separator(colors));

        let history = branch.to_owned();
        menu = menu.child(owned_menu_item(
            "Reveal in History".into(),
            true,
            None,
            colors,
            cx,
            move |app, _, cx| app.show_ref_history(history.clone(), cx),
        ));
        let copy = branch.to_owned();
        menu.child(owned_menu_item(
            "Copy Branch Name to Clipboard".into(),
            true,
            None,
            colors,
            cx,
            move |app, _, cx| app.copy_text_to_clipboard(&copy, cx),
        ))
    }

    fn extend_remote_branch_menu(
        mut menu: gpui::Stateful<gpui::Div>,
        branch: &str,
        colors: &ThemeColors,
        cx: &mut gpui::Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let named = NamedRefLabel::new(branch);
        let create_start = branch.to_owned();
        menu = menu.child(owned_menu_item(
            format!("Create New Branch from {}…", named.quoted()),
            true,
            None,
            colors,
            cx,
            move |app, _, cx| app.prompt_create_branch_from_ref(create_start.clone(), cx),
        ));
        let history = branch.to_owned();
        menu = menu.child(owned_menu_item(
            "Reveal in History".into(),
            true,
            None,
            colors,
            cx,
            move |app, _, cx| app.show_ref_history(history.clone(), cx),
        ));
        menu = menu.child(menu_separator(colors));
        let pull_branch = branch.to_owned();
        menu = menu.child(owned_menu_item(
            "Pull…".into(),
            true,
            None,
            colors,
            cx,
            move |app, _, cx| app.pull_branch(pull_branch.clone(), cx),
        ));
        menu = menu.child(menu_separator(colors));
        let delete_ref = branch.to_owned();
        menu = menu.child(owned_menu_item(
            format!("Delete {}…", named.quoted()),
            true,
            None,
            colors,
            cx,
            move |app, _, cx| app.request_remote_branch_delete(&delete_ref, cx),
        ));
        let copy = branch.to_owned();
        menu.child(owned_menu_item(
            "Copy Branch Name to Clipboard".into(),
            true,
            None,
            colors,
            cx,
            move |app, _, cx| app.copy_text_to_clipboard(&copy, cx),
        ))
    }

    fn extend_tag_menu(
        mut menu: gpui::Stateful<gpui::Div>,
        tag: &str,
        colors: &ThemeColors,
        cx: &mut gpui::Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let named = NamedRefLabel::new(tag);
        let create_start = tag.to_owned();
        menu = menu.child(owned_menu_item(
            format!("Create New Branch from {}…", named.quoted()),
            true,
            None,
            colors,
            cx,
            move |app, _, cx| app.prompt_create_branch_from_ref(create_start.clone(), cx),
        ));
        let history = tag.to_owned();
        menu = menu.child(owned_menu_item(
            "Reveal in History".into(),
            true,
            None,
            colors,
            cx,
            move |app, _, cx| app.show_ref_history(history.clone(), cx),
        ));
        menu = menu.child(menu_separator(colors));
        let delete_tag = tag.to_owned();
        menu = menu.child(owned_menu_item(
            format!("Delete {}…", named.quoted()),
            true,
            None,
            colors,
            cx,
            move |app, _, cx| app.delete_tag(delete_tag.clone(), cx),
        ));
        let copy = tag.to_owned();
        menu.child(owned_menu_item(
            "Copy Tag Name to Clipboard".into(),
            true,
            None,
            colors,
            cx,
            move |app, _, cx| app.copy_text_to_clipboard(&copy, cx),
        ))
    }

    fn extend_remote_menu(
        menu: gpui::Stateful<gpui::Div>,
        remote: &str,
        colors: &ThemeColors,
        cx: &mut gpui::Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let fetch_remote = remote.to_owned();
        menu.child(owned_menu_item(
            format!("Fetch {}", quote(remote)),
            true,
            None,
            colors,
            cx,
            move |app, _, cx| {
                app.run_network_command(
                    format!("Fetching {fetch_remote}"),
                    vec![
                        "fetch".into(),
                        "--progress".into(),
                        fetch_remote.clone().into(),
                    ],
                    cx,
                );
            },
        ))
    }

    pub(crate) fn ref_context_submenu_view(
        &self,
        colors: &ThemeColors,
        cx: &mut gpui::Context<Self>,
    ) -> Option<AnyElement> {
        let submenu = self.ref_context_submenu?;
        let RefContext::LocalBranch(branch) = self.ref_context.clone()? else {
            return None;
        };
        let mut panel = menu_shell(colors);
        match submenu {
            RefContextSubmenu::PushTo => {
                let remotes = self.configured_remote_names();
                if remotes.is_empty() {
                    panel = panel.child(disabled_hint("No remotes configured.", colors));
                } else {
                    for remote in remotes {
                        let destination = format!("{remote}/{branch}");
                        let label = destination.clone();
                        panel = panel.child(owned_menu_item(
                            label,
                            true,
                            None,
                            colors,
                            cx,
                            move |app, _, cx| {
                                app.push_branch_to_destination(&destination, cx);
                            },
                        ));
                    }
                }
            }
            RefContextSubmenu::TrackUpstream => {
                let remotes = self.remote_branch_choices();
                if remotes.is_empty() {
                    panel = panel.child(disabled_hint("No remote branches to track.", colors));
                } else {
                    for upstream in remotes {
                        let track_branch = branch.clone();
                        let track_upstream = upstream.clone();
                        panel = panel.child(owned_menu_item(
                            upstream,
                            true,
                            None,
                            colors,
                            cx,
                            move |app, _, cx| {
                                app.set_branch_upstream(&track_branch, &track_upstream, cx);
                            },
                        ));
                    }
                }
            }
        }
        Some(panel.into_any_element())
    }

    fn local_branch_upstream(&self, branch: &str) -> Option<String> {
        self.refs.local_branches.iter().find_map(|reference| {
            let name = String::from_utf8(reference.name.0.clone()).ok()?;
            (name == branch)
                .then(|| reference.upstream.clone())
                .flatten()
        })
    }

    fn can_create_pull_request(&self) -> bool {
        self.service_account.is_some() || self.pull_request_repository.is_some()
    }
}

struct NamedRefLabel<'a> {
    name: &'a str,
}

impl<'a> NamedRefLabel<'a> {
    fn new(name: &'a str) -> Self {
        Self { name }
    }

    fn quoted(&self) -> String {
        quote(self.name)
    }
}

fn quote(name: &str) -> String {
    format!("\"{name}\"")
}

fn menu_shell(colors: &ThemeColors) -> gpui::Stateful<gpui::Div> {
    div()
        .id("ref-context-menu")
        .debug_selector(|| "ref-context-menu".into())
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

fn disabled_hint(label: &str, colors: &ThemeColors) -> AnyElement {
    div()
        .h(px(24.0))
        .px_2()
        .flex()
        .items_center()
        .text_sm()
        .text_color(colors.text_muted)
        .child(label.to_owned())
        .into_any_element()
}

fn submenu_item(
    label: String,
    submenu: RefContextSubmenu,
    active: bool,
    colors: &ThemeColors,
    cx: &mut gpui::Context<GitronimoApp>,
) -> AnyElement {
    // `active` is passed in from the parent `&self` during Render. Do not
    // `cx.entity().read(cx)` here: Render already holds a GitronimoApp lease,
    // and a second read panics (`entity_map` double_lease).
    let open = submenu;
    div()
        .id(gpui::ElementId::Name(format!("submenu:{label}").into()))
        .h(px(24.0))
        .px_2()
        .flex()
        .items_center()
        .justify_between()
        .gap_3()
        .text_sm()
        .rounded(px(3.0))
        .cursor_pointer()
        .bg(if active {
            colors.selection
        } else {
            colors.panel_background
        })
        .hover(|style| style.bg(colors.selection))
        .on_click(cx.listener(move |app, _: &ClickEvent, _, cx| {
            app.open_ref_context_submenu(open, cx);
        }))
        .child(label)
        .child(div().text_xs().text_color(colors.text_muted).child("▸"))
        .into_any_element()
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
            format!("context-menu-item:{label}").into(),
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
                app.close_ref_context_menu(cx);
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

/// Anchored overlay for the ref context menu and its optional flyout.
impl GitronimoApp {
    pub(crate) fn ref_context_menu_overlay(
        &self,
        colors: &ThemeColors,
        cx: &mut gpui::Context<Self>,
    ) -> AnyElement {
        let (x, y) = self.ref_context_menu_position.unwrap_or((12.0, 90.0));
        let submenu = self.ref_context_submenu_view(colors, cx);
        div()
            .absolute()
            .inset_0()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|app, _: &MouseDownEvent, _, cx| {
                    app.close_ref_context_menu(cx);
                }),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|app, _: &MouseDownEvent, _, cx| {
                    app.close_ref_context_menu(cx);
                }),
            )
            .child(
                gpui::anchored()
                    .position(gpui::point(px(x), px(y)))
                    .anchor(gpui::Corner::TopLeft)
                    .snap_to_window_with_margin(gpui::Edges::all(px(8.0)))
                    .child(
                        div()
                            .flex()
                            .items_start()
                            .gap_1()
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
                            .children(self.ref_context_menu_view(colors, cx))
                            .children(submenu),
                    ),
            )
            .into_any_element()
    }
}
