//! GPUI single-line text field with IME/paste support (adapted from gpui `examples/input.rs`).

use std::ops::Range;

use gpui::{
    App, Bounds, ClipboardItem, Context, CursorStyle, Element, ElementInputHandler, Entity,
    EntityInputHandler, FocusHandle, Focusable, GlobalElementId, KeyBinding, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, PaintQuad, Pixels, Point, Render, ShapedLine,
    SharedString, TextRun, UTF16Selection, UnderlineStyle, Window, actions, div, fill, point,
    prelude::*, px, relative, size,
};
use ui_kit::ThemeColors;

use crate::app_state::GitronimoApp;

actions!(
    single_line_input,
    [
        Backspace,
        Delete,
        Left,
        Right,
        SelectLeft,
        SelectRight,
        SelectAll,
        Home,
        End,
        Paste,
        Cut,
        Copy,
        Confirm,
        Cancel,
        MoveUp,
        MoveDown,
    ]
);

/// Which app string field this input edits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TextFieldBinding {
    WelcomeSearch,
    WorktreeSearch,
    CommitSubject,
    CommitBody,
    RepoDescription,
    TextPrompt,
    CommandPalette,
}

pub(crate) struct SingleLineInput {
    focus_handle: FocusHandle,
    binding: TextFieldBinding,
    app: Entity<GitronimoApp>,
    placeholder: String,
    selected_range: Range<usize>,
    selection_reversed: bool,
    marked_range: Option<Range<usize>>,
    last_layout: Option<ShapedLine>,
    last_bounds: Option<Bounds<Pixels>>,
    is_selecting: bool,
}

impl SingleLineInput {
    pub(crate) fn new(
        binding: TextFieldBinding,
        app: Entity<GitronimoApp>,
        placeholder: impl Into<String>,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            binding,
            app,
            placeholder: placeholder.into(),
            selected_range: 0..0,
            selection_reversed: false,
            marked_range: None,
            last_layout: None,
            last_bounds: None,
            is_selecting: false,
        }
    }

    fn content_from_app(&self, cx: &App) -> String {
        self.app.read(cx).field_for_binding(self.binding).to_owned()
    }

    fn write_content(&mut self, content: String, cx: &mut Context<Self>) {
        self.app.update(cx, |app, cx| {
            app.set_field_for_binding(self.binding, content);
            cx.notify();
        });
    }

    fn sync_from_app(&mut self, cx: &App) {
        let len = self.content_from_app(cx).len();
        if self.cursor_offset() > len {
            self.selected_range = len..len;
        }
    }

    fn cursor_offset(&self) -> usize {
        if self.selection_reversed {
            self.selected_range.start
        } else {
            self.selected_range.end
        }
    }

    fn move_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        self.selected_range = offset..offset;
        cx.notify();
    }

    fn select_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        if self.selection_reversed {
            self.selected_range.start = offset;
        } else {
            self.selected_range.end = offset;
        }
        if self.selected_range.end < self.selected_range.start {
            self.selection_reversed = !self.selection_reversed;
            self.selected_range = self.selected_range.end..self.selected_range.start;
        }
        cx.notify();
    }

    fn left(&mut self, _: &Left, _: &mut Window, cx: &mut Context<Self>) {
        let content = self.content_from_app(cx);
        if self.selected_range.is_empty() {
            self.move_to(Self::previous_boundary(&content, self.cursor_offset()), cx);
        } else {
            self.move_to(self.selected_range.start, cx);
        }
    }

    fn right(&mut self, _: &Right, _: &mut Window, cx: &mut Context<Self>) {
        let content = self.content_from_app(cx);
        if self.selected_range.is_empty() {
            self.move_to(Self::next_boundary(&content, self.selected_range.end), cx);
        } else {
            self.move_to(self.selected_range.end, cx);
        }
    }

    fn select_left(&mut self, _: &SelectLeft, _: &mut Window, cx: &mut Context<Self>) {
        let content = self.content_from_app(cx);
        self.select_to(Self::previous_boundary(&content, self.cursor_offset()), cx);
    }

    fn select_right(&mut self, _: &SelectRight, _: &mut Window, cx: &mut Context<Self>) {
        let content = self.content_from_app(cx);
        self.select_to(Self::next_boundary(&content, self.cursor_offset()), cx);
    }

    fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        let len = self.content_from_app(cx).len();
        self.move_to(0, cx);
        self.select_to(len, cx);
    }

    fn home(&mut self, _: &Home, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(0, cx);
    }

    fn end(&mut self, _: &End, _: &mut Window, cx: &mut Context<Self>) {
        let len = self.content_from_app(cx).len();
        self.move_to(len, cx);
    }

    fn backspace(&mut self, _: &Backspace, window: &mut Window, cx: &mut Context<Self>) {
        let content = self.content_from_app(cx);
        if self.selected_range.is_empty() {
            self.select_to(Self::previous_boundary(&content, self.cursor_offset()), cx);
        }
        self.replace_text_in_range(None, "", window, cx);
    }

    fn delete(&mut self, _: &Delete, window: &mut Window, cx: &mut Context<Self>) {
        let content = self.content_from_app(cx);
        if self.selected_range.is_empty() {
            self.select_to(Self::next_boundary(&content, self.cursor_offset()), cx);
        }
        self.replace_text_in_range(None, "", window, cx);
    }

    fn paste(&mut self, _: &Paste, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            let sanitized = if matches!(self.binding, TextFieldBinding::CommitBody) {
                text
            } else {
                text.replace('\n', " ")
            };
            self.replace_text_in_range(None, &sanitized, window, cx);
        }
    }

    fn copy_selection(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        let content = self.content_from_app(cx);
        if !self.selected_range.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(
                content[self.selected_range.clone()].to_string(),
            ));
        }
    }

    fn cut(&mut self, _: &Cut, window: &mut Window, cx: &mut Context<Self>) {
        self.copy_selection(&Copy, window, cx);
        self.replace_text_in_range(None, "", window, cx);
    }

    fn confirm(&mut self, _: &Confirm, _: &mut Window, cx: &mut Context<Self>) {
        match self.binding {
            TextFieldBinding::TextPrompt => {
                self.app.update(cx, GitronimoApp::confirm_text_prompt);
            }
            TextFieldBinding::CommandPalette => {
                self.app.update(cx, GitronimoApp::confirm_command_palette);
            }
            _ => {}
        }
    }

    fn cancel(&mut self, _: &Cancel, _: &mut Window, cx: &mut Context<Self>) {
        match self.binding {
            TextFieldBinding::TextPrompt => {
                self.app.update(cx, GitronimoApp::cancel_text_prompt);
            }
            TextFieldBinding::CommandPalette => {
                self.app.update(cx, GitronimoApp::close_command_palette);
            }
            _ => {}
        }
    }

    fn move_up(&mut self, _: &MoveUp, _: &mut Window, cx: &mut Context<Self>) {
        if matches!(self.binding, TextFieldBinding::CommandPalette) {
            self.app.update(cx, |app, cx| {
                app.move_command_palette_selection(-1, cx);
            });
        }
    }

    fn move_down(&mut self, _: &MoveDown, _: &mut Window, cx: &mut Context<Self>) {
        if matches!(self.binding, TextFieldBinding::CommandPalette) {
            self.app.update(cx, |app, cx| {
                app.move_command_palette_selection(1, cx);
            });
        }
    }

    fn on_mouse_down(&mut self, event: &MouseDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.is_selecting = true;
        let index = self.index_for_mouse_position(event.position);
        if event.modifiers.shift {
            self.select_to(index, cx);
        } else {
            self.move_to(index, cx);
        }
    }

    fn on_mouse_up(&mut self, _: &MouseUpEvent, _: &mut Window, _: &mut Context<Self>) {
        self.is_selecting = false;
    }

    fn on_mouse_move(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.is_selecting {
            self.select_to(self.index_for_mouse_position(event.position), cx);
        }
    }

    fn index_for_mouse_position(&self, position: Point<Pixels>) -> usize {
        let (Some(bounds), Some(line)) = (self.last_bounds.as_ref(), self.last_layout.as_ref())
        else {
            return 0;
        };
        if position.y < bounds.top() {
            return 0;
        }
        if position.y > bounds.bottom() {
            return line.len();
        }
        line.closest_index_for_x(position.x - bounds.left())
    }

    fn previous_boundary(content: &str, offset: usize) -> usize {
        content
            .char_indices()
            .rev()
            .find_map(|(idx, _)| (idx < offset).then_some(idx))
            .unwrap_or(0)
    }

    fn next_boundary(content: &str, offset: usize) -> usize {
        content
            .char_indices()
            .find_map(|(idx, _)| (idx > offset).then_some(idx))
            .unwrap_or(content.len())
    }

    fn offset_from_utf16(content: &str, offset: usize) -> usize {
        let mut utf8_offset = 0;
        let mut utf16_count = 0;
        for ch in content.chars() {
            if utf16_count >= offset {
                break;
            }
            utf16_count += ch.len_utf16();
            utf8_offset += ch.len_utf8();
        }
        utf8_offset
    }

    fn offset_to_utf16(content: &str, offset: usize) -> usize {
        let mut utf16_offset = 0;
        let mut utf8_count = 0;
        for ch in content.chars() {
            if utf8_count >= offset {
                break;
            }
            utf8_count += ch.len_utf8();
            utf16_offset += ch.len_utf16();
        }
        utf16_offset
    }

    fn range_to_utf16(content: &str, range: &Range<usize>) -> Range<usize> {
        Self::offset_to_utf16(content, range.start)..Self::offset_to_utf16(content, range.end)
    }

    fn range_from_utf16(content: &str, range_utf16: &Range<usize>) -> Range<usize> {
        Self::offset_from_utf16(content, range_utf16.start)
            ..Self::offset_from_utf16(content, range_utf16.end)
    }
}

impl EntityInputHandler for SingleLineInput {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        actual_range: &mut Option<Range<usize>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<String> {
        let content = self.content_from_app(cx);
        let range = Self::range_from_utf16(&content, &range_utf16);
        actual_range.replace(Self::range_to_utf16(&content, &range));
        Some(content[range].to_string())
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        let content = self.content_from_app(cx);
        Some(UTF16Selection {
            range: Self::range_to_utf16(&content, &self.selected_range),
            reversed: self.selection_reversed,
        })
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        let content = self.content_from_app(cx);
        self.marked_range
            .as_ref()
            .map(|range| Self::range_to_utf16(&content, range))
    }

    fn unmark_text(&mut self, _window: &mut Window, _: &mut Context<Self>) {
        self.marked_range = None;
    }

    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mut content = self.content_from_app(cx);
        let range = range_utf16
            .as_ref()
            .map(|r| Self::range_from_utf16(&content, r))
            .or_else(|| self.marked_range.clone())
            .unwrap_or_else(|| self.selected_range.clone());
        content = format!(
            "{}{}{}",
            &content[..range.start],
            new_text,
            &content[range.end..]
        );
        let cursor = range.start + new_text.len();
        self.selected_range = cursor..cursor;
        self.marked_range = None;
        self.write_content(content, cx);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range_utf16: Option<Range<usize>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mut content = self.content_from_app(cx);
        let range = range_utf16
            .as_ref()
            .map(|r| Self::range_from_utf16(&content, r))
            .or_else(|| self.marked_range.clone())
            .unwrap_or_else(|| self.selected_range.clone());
        content = format!(
            "{}{}{}",
            &content[..range.start],
            new_text,
            &content[range.end..]
        );
        self.marked_range =
            (!new_text.is_empty()).then_some(range.start..range.start + new_text.len());
        self.selected_range = if let Some(r) = new_selected_range_utf16.as_ref() {
            let new_range = Self::range_from_utf16(&content, r);
            new_range.start + range.start..new_range.end + range.end
        } else {
            let cursor = range.start + new_text.len();
            cursor..cursor
        };
        self.write_content(content, cx);
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        bounds: Bounds<Pixels>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let content = self.content_from_app(cx);
        let last_layout = self.last_layout.as_ref()?;
        let range = Self::range_from_utf16(&content, &range_utf16);
        Some(Bounds::from_corners(
            point(
                bounds.left() + last_layout.x_for_index(range.start),
                bounds.top(),
            ),
            point(
                bounds.left() + last_layout.x_for_index(range.end),
                bounds.bottom(),
            ),
        ))
    }

    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<usize> {
        let content = self.content_from_app(cx);
        let line_point = self.last_bounds?.localize(&point)?;
        let last_layout = self.last_layout.as_ref()?;
        let utf8_index = last_layout.index_for_x(point.x - line_point.x)?;
        Some(Self::offset_to_utf16(&content, utf8_index))
    }
}

struct InputElement {
    input: Entity<SingleLineInput>,
    colors: ThemeColors,
}

struct PrepaintState {
    line: Option<ShapedLine>,
    cursor: Option<PaintQuad>,
    selection: Option<PaintQuad>,
}

impl IntoElement for InputElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for InputElement {
    type RequestLayoutState = ();
    type PrepaintState = PrepaintState;

    fn id(&self) -> Option<gpui::ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (gpui::LayoutId, Self::RequestLayoutState) {
        let mut style = gpui::Style::default();
        style.size.width = relative(1.).into();
        style.size.height = window.line_height().into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        (): &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let input = self.input.read(cx);
        let content = input.content_from_app(cx);
        let placeholder = input.placeholder.clone();
        let selected_range = input.selected_range.clone();
        let cursor = input.cursor_offset();
        let style = window.text_style();
        let colors = self.colors;

        let (display_text, text_color) = if content.is_empty() {
            (SharedString::from(placeholder), colors.text_muted.into())
        } else {
            (SharedString::from(content), colors.text_primary.into())
        };

        let run = TextRun {
            len: display_text.len(),
            font: style.font(),
            color: text_color,
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let runs = if let Some(marked_range) = input.marked_range.as_ref() {
            vec![
                TextRun {
                    len: marked_range.start,
                    ..run.clone()
                },
                TextRun {
                    len: marked_range.end - marked_range.start,
                    underline: Some(UnderlineStyle {
                        color: Some(run.color),
                        thickness: px(1.0),
                        wavy: false,
                    }),
                    ..run.clone()
                },
                TextRun {
                    len: display_text.len().saturating_sub(marked_range.end),
                    ..run
                },
            ]
            .into_iter()
            .filter(|r| r.len > 0)
            .collect()
        } else {
            vec![run]
        };

        let font_size = style.font_size.to_pixels(window.rem_size());
        let line = window
            .text_system()
            .shape_line(display_text, font_size, &runs, None);

        let cursor_pos = line.x_for_index(cursor);
        let (selection, cursor_quad) = if selected_range.is_empty() {
            (
                None,
                Some(fill(
                    Bounds::new(
                        point(bounds.left() + cursor_pos, bounds.top()),
                        size(px(1.5), bounds.bottom() - bounds.top()),
                    ),
                    colors.accent,
                )),
            )
        } else {
            (
                Some(fill(
                    Bounds::from_corners(
                        point(
                            bounds.left() + line.x_for_index(selected_range.start),
                            bounds.top(),
                        ),
                        point(
                            bounds.left() + line.x_for_index(selected_range.end),
                            bounds.bottom(),
                        ),
                    ),
                    colors.selection,
                )),
                None,
            )
        };
        PrepaintState {
            line: Some(line),
            cursor: cursor_quad,
            selection,
        }
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        (): &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let focus_handle = self.input.read(cx).focus_handle.clone();
        window.handle_input(
            &focus_handle,
            ElementInputHandler::new(bounds, self.input.clone()),
            cx,
        );
        if let Some(selection) = prepaint.selection.take() {
            window.paint_quad(selection);
        }
        let line = prepaint.line.take().unwrap();
        line.paint(bounds.origin, window.line_height(), window, cx)
            .unwrap();
        if focus_handle.is_focused(window)
            && let Some(cursor) = prepaint.cursor.take()
        {
            window.paint_quad(cursor);
        }
        self.input.update(cx, |input, _| {
            input.last_layout = Some(line);
            input.last_bounds = Some(bounds);
        });
    }
}

impl Render for SingleLineInput {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.sync_from_app(cx);
        let colors = ui_kit::Theme::for_appearance(self.app.read(cx).appearance).colors;
        div()
            .flex()
            .w_full()
            .key_context("SingleLineInput")
            .track_focus(&self.focus_handle)
            .cursor(CursorStyle::IBeam)
            .on_action(cx.listener(Self::backspace))
            .on_action(cx.listener(Self::delete))
            .on_action(cx.listener(Self::left))
            .on_action(cx.listener(Self::right))
            .on_action(cx.listener(Self::select_left))
            .on_action(cx.listener(Self::select_right))
            .on_action(cx.listener(Self::select_all))
            .on_action(cx.listener(Self::home))
            .on_action(cx.listener(Self::end))
            .on_action(cx.listener(Self::paste))
            .on_action(cx.listener(Self::cut))
            .on_action(cx.listener(Self::copy_selection))
            .on_action(cx.listener(Self::confirm))
            .on_action(cx.listener(Self::cancel))
            .on_action(cx.listener(Self::move_up))
            .on_action(cx.listener(Self::move_down))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .child(InputElement {
                input: cx.entity(),
                colors,
            })
    }
}

impl Focusable for SingleLineInput {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

/// Render a single-line input entity inside a styled container.
pub(crate) fn single_line_input_shell(
    input: Entity<SingleLineInput>,
    colors: &ThemeColors,
    rounded: bool,
) -> impl IntoElement {
    div()
        .flex_1()
        .min_w(px(80.0))
        .h(px(24.0))
        .px_2()
        .flex()
        .items_center()
        .bg(if rounded {
            colors.search_field_background
        } else {
            colors.raised_background
        })
        .when(rounded, gpui::Styled::rounded_full)
        .when(!rounded, |el| el.rounded(px(4.0)))
        .border_1()
        .border_color(colors.border)
        .child(input)
}

pub(crate) type TextInputBundle = (
    Entity<SingleLineInput>,
    Entity<SingleLineInput>,
    Entity<SingleLineInput>,
    Entity<SingleLineInput>,
    Entity<SingleLineInput>,
    Entity<SingleLineInput>,
    Entity<SingleLineInput>,
);

impl GitronimoApp {
    pub(crate) fn field_for_binding(&self, binding: TextFieldBinding) -> &str {
        match binding {
            TextFieldBinding::WelcomeSearch => self.welcome_repo_search.as_str(),
            TextFieldBinding::WorktreeSearch => self.worktree_file_search.as_str(),
            TextFieldBinding::CommitSubject => self.commit_subject.as_str(),
            TextFieldBinding::CommitBody => self.commit_body.as_str(),
            TextFieldBinding::RepoDescription => self.user_repo_description.as_str(),
            TextFieldBinding::TextPrompt => self.text_prompt_value.as_str(),
            TextFieldBinding::CommandPalette => self.command_palette_query.as_str(),
        }
    }

    pub(crate) fn set_field_for_binding(&mut self, binding: TextFieldBinding, value: String) {
        match binding {
            TextFieldBinding::WelcomeSearch => self.welcome_repo_search = value,
            TextFieldBinding::WorktreeSearch => self.worktree_file_search = value,
            TextFieldBinding::CommitSubject => {
                self.commit_subject = value;
                self.commit_subject_focused = true;
            }
            TextFieldBinding::CommitBody => self.commit_body = value,
            TextFieldBinding::RepoDescription => self.user_repo_description = value,
            TextFieldBinding::TextPrompt => self.text_prompt_value = value,
            TextFieldBinding::CommandPalette => {
                self.command_palette_query = value;
                self.command_palette_selected = 0;
            }
        }
    }

    pub(crate) fn create_text_inputs(cx: &mut Context<Self>) -> TextInputBundle {
        let app = cx.entity();
        let welcome = cx.new(|cx| {
            SingleLineInput::new(
                TextFieldBinding::WelcomeSearch,
                app.clone(),
                "Search repositories",
                cx,
            )
        });
        let worktree = cx.new(|cx| {
            SingleLineInput::new(
                TextFieldBinding::WorktreeSearch,
                app.clone(),
                "Search for File",
                cx,
            )
        });
        let subject = cx.new(|cx| {
            SingleLineInput::new(
                TextFieldBinding::CommitSubject,
                app.clone(),
                "Commit Subject",
                cx,
            )
        });
        let body = cx.new(|cx| {
            SingleLineInput::new(TextFieldBinding::CommitBody, app.clone(), "Description", cx)
        });
        let description = cx.new(|cx| {
            SingleLineInput::new(
                TextFieldBinding::RepoDescription,
                app.clone(),
                "Add a description…",
                cx,
            )
        });
        let text_prompt = cx.new(|cx| {
            SingleLineInput::new(TextFieldBinding::TextPrompt, app.clone(), "Enter value", cx)
        });
        let command_palette = cx.new(|cx| {
            SingleLineInput::new(TextFieldBinding::CommandPalette, app, "Filter commands", cx)
        });
        (
            welcome,
            worktree,
            subject,
            body,
            description,
            text_prompt,
            command_palette,
        )
    }
}

pub(crate) fn register_input_bindings(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("backspace", Backspace, Some("SingleLineInput")),
        KeyBinding::new("delete", Delete, Some("SingleLineInput")),
        KeyBinding::new("left", Left, Some("SingleLineInput")),
        KeyBinding::new("right", Right, Some("SingleLineInput")),
        KeyBinding::new("shift-left", SelectLeft, Some("SingleLineInput")),
        KeyBinding::new("shift-right", SelectRight, Some("SingleLineInput")),
        KeyBinding::new("cmd-a", SelectAll, Some("SingleLineInput")),
        KeyBinding::new("home", Home, Some("SingleLineInput")),
        KeyBinding::new("end", End, Some("SingleLineInput")),
        KeyBinding::new("cmd-v", Paste, Some("SingleLineInput")),
        KeyBinding::new("cmd-c", Copy, Some("SingleLineInput")),
        KeyBinding::new("cmd-x", Cut, Some("SingleLineInput")),
        KeyBinding::new("enter", Confirm, Some("SingleLineInput")),
        KeyBinding::new("escape", Cancel, Some("SingleLineInput")),
        KeyBinding::new("up", MoveUp, Some("SingleLineInput")),
        KeyBinding::new("down", MoveDown, Some("SingleLineInput")),
    ]);
}
