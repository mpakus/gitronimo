use gpui::actions;

actions!(
    gitronimo,
    [
        OpenRepository,
        Refresh,
        FocusComposer,
        HistoryPrevious,
        HistoryNext,
        NavigateBack,
        NavigateForward,
        CommandPalette,
        ShortcutReference,
        ToggleAppearance,
        WidenSidebar,
        WidenInspector,
        SelectAllStatusFiles,
        SaveStash,
        Quit
    ]
);
