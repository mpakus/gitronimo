use gpui::KeyBinding;

use crate::actions::{
    CommandPalette, FocusComposer, HistoryNext, HistoryPrevious, NavigateBack, NavigateForward,
    OpenRepository, Quit, Refresh, SaveStash, SelectAllStatusFiles, ShortcutReference,
    ToggleAppearance, WidenInspector, WidenSidebar,
};

pub fn bindings() -> [KeyBinding; 15] {
    [
        KeyBinding::new("cmd-q", Quit, None),
        KeyBinding::new("cmd-o", OpenRepository, None),
        KeyBinding::new("cmd-r", Refresh, None),
        KeyBinding::new("cmd-shift-c", FocusComposer, None),
        KeyBinding::new("cmd-shift-s", SaveStash, None),
        KeyBinding::new("cmd-shift-p", CommandPalette, None),
        KeyBinding::new("cmd-/", ShortcutReference, None),
        KeyBinding::new("cmd-[", NavigateBack, None),
        KeyBinding::new("cmd-]", NavigateForward, None),
        KeyBinding::new("up", HistoryPrevious, None),
        KeyBinding::new("down", HistoryNext, None),
        KeyBinding::new("cmd-shift-l", ToggleAppearance, None),
        KeyBinding::new("cmd-alt-left", WidenSidebar, None),
        KeyBinding::new("cmd-alt-right", WidenInspector, None),
        KeyBinding::new("cmd-a", SelectAllStatusFiles, None),
    ]
}
