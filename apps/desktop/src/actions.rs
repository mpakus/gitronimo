use gpui::actions;

actions!(
    gitronimo,
    [
        OpenRepository,
        Refresh,
        FocusComposer,
        FocusSearch,
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
        Hide,
        About,
        CheckForUpdates,
        OpenSettings,
        Quit
    ]
);
