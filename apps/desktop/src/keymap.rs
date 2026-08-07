use gpui::KeyBinding;

use crate::actions::{OpenRepository, Refresh, ToggleAppearance, WidenInspector, WidenSidebar};

pub fn bindings() -> [KeyBinding; 5] {
    [
        KeyBinding::new("cmd-o", OpenRepository, None),
        KeyBinding::new("cmd-r", Refresh, None),
        KeyBinding::new("cmd-shift-l", ToggleAppearance, None),
        KeyBinding::new("cmd-alt-left", WidenSidebar, None),
        KeyBinding::new("cmd-alt-right", WidenInspector, None),
    ]
}
