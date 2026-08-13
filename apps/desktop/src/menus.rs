use gpui::{Menu, MenuItem};

use crate::actions::{
    CommandPalette, FocusSearch, Hide, NavigateBack, NavigateForward, OpenRepository, Quit,
    Refresh, ShortcutReference, ToggleAppearance,
};

pub fn application_menus() -> Vec<Menu> {
    vec![
        Menu {
            name: "Gitronimo".into(),
            items: vec![
                MenuItem::action("Hide Gitronimo", Hide),
                MenuItem::action("Quit Gitronimo", Quit),
            ],
        },
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
                MenuItem::action("Find", FocusSearch),
                MenuItem::action("Keyboard Shortcuts", ShortcutReference),
                MenuItem::action("Toggle Appearance", ToggleAppearance),
            ],
        },
    ]
}
