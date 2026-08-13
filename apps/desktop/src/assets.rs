//! Embedded SVG assets for GPUI `svg().path(...)` (no extra crate deps).

use std::borrow::Cow;

use gpui::{AssetSource, Result, SharedString};

/// Loads icons from `apps/desktop/assets/icons/` via `include_bytes!`.
pub(crate) struct DesktopAssets;

impl AssetSource for DesktopAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        let bytes = match path {
            "icons/folder.svg" => include_bytes!("../assets/icons/folder.svg").as_slice(),
            "icons/chevron-right.svg" => {
                include_bytes!("../assets/icons/chevron-right.svg").as_slice()
            }
            "icons/chevron-left.svg" => {
                include_bytes!("../assets/icons/chevron-left.svg").as_slice()
            }
            "icons/chevron-down.svg" => {
                include_bytes!("../assets/icons/chevron-down.svg").as_slice()
            }
            "icons/plus.svg" => include_bytes!("../assets/icons/plus.svg").as_slice(),
            "icons/magnifying-glass.svg" => {
                include_bytes!("../assets/icons/magnifying-glass.svg").as_slice()
            }
            "icons/cloud.svg" => include_bytes!("../assets/icons/cloud.svg").as_slice(),
            "icons/bookmark.svg" => include_bytes!("../assets/icons/bookmark.svg").as_slice(),
            "icons/arrow-path.svg" => include_bytes!("../assets/icons/arrow-path.svg").as_slice(),
            "icons/cube.svg" => include_bytes!("../assets/icons/cube.svg").as_slice(),
            "icons/command-line.svg" => {
                include_bytes!("../assets/icons/command-line.svg").as_slice()
            }
            "icons/squares-2x2.svg" => include_bytes!("../assets/icons/squares-2x2.svg").as_slice(),
            "icons/arrow-down-tray.svg" => {
                include_bytes!("../assets/icons/arrow-down-tray.svg").as_slice()
            }
            "icons/arrow-up-tray.svg" => {
                include_bytes!("../assets/icons/arrow-up-tray.svg").as_slice()
            }
            "icons/arrow-uturn-left.svg" => {
                include_bytes!("../assets/icons/arrow-uturn-left.svg").as_slice()
            }
            "icons/arrow-uturn-right.svg" => {
                include_bytes!("../assets/icons/arrow-uturn-right.svg").as_slice()
            }
            "icons/clock.svg" => include_bytes!("../assets/icons/clock.svg").as_slice(),
            "icons/cog-6-tooth.svg" => include_bytes!("../assets/icons/cog-6-tooth.svg").as_slice(),
            "icons/tag.svg" => include_bytes!("../assets/icons/tag.svg").as_slice(),
            "icons/document-text.svg" => {
                include_bytes!("../assets/icons/document-text.svg").as_slice()
            }
            "icons/arrows-right-left.svg" => {
                include_bytes!("../assets/icons/arrows-right-left.svg").as_slice()
            }
            "icons/scale.svg" => include_bytes!("../assets/icons/scale.svg").as_slice(),
            "icons/branch.svg" => include_bytes!("../assets/icons/branch.svg").as_slice(),
            "icons/gitronimo-icon.png" => {
                include_bytes!("../../../assets/gitronimo-icon.png").as_slice()
            }
            _ => return Ok(None),
        };
        Ok(Some(Cow::Borrowed(bytes)))
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let all = [
            "icons/folder.svg",
            "icons/chevron-right.svg",
            "icons/chevron-left.svg",
            "icons/chevron-down.svg",
            "icons/plus.svg",
            "icons/magnifying-glass.svg",
            "icons/cloud.svg",
            "icons/bookmark.svg",
            "icons/arrow-path.svg",
            "icons/cube.svg",
            "icons/command-line.svg",
            "icons/squares-2x2.svg",
            "icons/arrow-down-tray.svg",
            "icons/arrow-up-tray.svg",
            "icons/arrow-uturn-left.svg",
            "icons/arrow-uturn-right.svg",
            "icons/clock.svg",
            "icons/cog-6-tooth.svg",
            "icons/tag.svg",
            "icons/document-text.svg",
            "icons/arrows-right-left.svg",
            "icons/scale.svg",
            "icons/branch.svg",
            "icons/gitronimo-icon.png",
        ];
        Ok(all
            .into_iter()
            .filter(|entry| entry.starts_with(path))
            .map(SharedString::from)
            .collect())
    }
}
