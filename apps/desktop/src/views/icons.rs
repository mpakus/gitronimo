//! Presentational outline icons (Heroicons MIT + original branch glyph) for chrome UI.

use gpui::{AnyElement, Hsla, IntoElement, div, prelude::*, px, svg};

/// Named chrome icons mapped to vendored SVG paths under `assets/icons/`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum IconKind {
    Folder,
    ChevronRight,
    ChevronLeft,
    ChevronDown,
    Plus,
    Search,
    Cloud,
    Bookmark,
    Workflow,
    Repo,
    Palette,
    Grid,
    Fetch,
    Push,
    Pull,
    Sync,
    StashApply,
    StashSave,
    WorkingCopy,
    History,
    Stashes,
    Settings,
    Branch,
    Tag,
}

impl IconKind {
    fn path(self) -> &'static str {
        match self {
            Self::Folder => "icons/folder.svg",
            Self::ChevronRight => "icons/chevron-right.svg",
            Self::ChevronLeft => "icons/chevron-left.svg",
            Self::ChevronDown => "icons/chevron-down.svg",
            Self::Plus => "icons/plus.svg",
            Self::Search => "icons/magnifying-glass.svg",
            Self::Cloud => "icons/cloud.svg",
            Self::Bookmark => "icons/bookmark.svg",
            Self::Workflow | Self::Sync => "icons/arrow-path.svg",
            Self::Repo => "icons/cube.svg",
            Self::Palette => "icons/command-line.svg",
            Self::Grid => "icons/squares-2x2.svg",
            Self::Fetch | Self::Pull => "icons/arrow-down-tray.svg",
            Self::Push => "icons/arrow-up-tray.svg",
            Self::StashApply | Self::Stashes => "icons/arrow-uturn-left.svg",
            Self::StashSave => "icons/arrow-uturn-right.svg",
            Self::WorkingCopy => "icons/document-text.svg",
            Self::History => "icons/clock.svg",
            Self::Settings => "icons/cog-6-tooth.svg",
            Self::Branch => "icons/branch.svg",
            Self::Tag => "icons/tag.svg",
        }
    }
}

/// Render a sized outline icon tinted with `color` (via GPUI SVG alpha + text color).
pub(crate) fn icon(kind: IconKind, size: f32, color: impl Into<Hsla>) -> AnyElement {
    let color = color.into();
    div()
        .w(px(size))
        .h(px(size))
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_center()
        .child(
            svg()
                .path(kind.path())
                .size(px(size))
                .text_color(color)
                .flex_shrink_0(),
        )
        .into_any_element()
}
