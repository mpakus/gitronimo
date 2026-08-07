use gpui::{Menu, MenuItem};

use crate::actions::{OpenRepository, Refresh, ToggleAppearance};

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
                MenuItem::action("Toggle Appearance", ToggleAppearance),
            ],
        },
    ]
}
