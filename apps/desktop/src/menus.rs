use gpui::{Menu, MenuItem};

use crate::actions::{
    CommandPalette, NavigateBack, NavigateForward, OpenRepository, Refresh, ShortcutReference,
    ToggleAppearance,
};

pub fn application_menus() -> Vec<Menu> {
    vec![
        Menu {
            name: "File".into(),
            items: vec![MenuItem::action("Open Repository…", OpenRepository)],
        },
        Menu {
            name: "View".into(),
            items: vec![
                MenuItem::action("Refresh", Refresh),
                MenuItem::action("Back", NavigateBack),
                MenuItem::action("Forward", NavigateForward),
                MenuItem::action("Command Palette", CommandPalette),
                MenuItem::action("Keyboard Shortcuts", ShortcutReference),
                MenuItem::action("Toggle Appearance", ToggleAppearance),
            ],
        },
    ]
}
