use gpui::{Menu, MenuItem};

use crate::actions::{
    About, CommandPalette, FocusSearch, Hide, NavigateBack, NavigateForward, OpenRepository, Quit,
    Refresh, ShortcutReference, ToggleAppearance,
};

pub fn application_menus() -> Vec<Menu> {
    vec![
        Menu {
            name: "GitRonimo".into(),
            items: vec![
                MenuItem::action("About GitRonimo", About),
                MenuItem::separator(),
                MenuItem::action("Hide GitRonimo", Hide),
                MenuItem::action("Quit GitRonimo", Quit),
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
