use gpui::actions;

actions!(
    gitronimo,
    [
        OpenRepository,
        Refresh,
        FocusComposer,
        HistoryPrevious,
        HistoryNext,
        ToggleAppearance,
        WidenSidebar,
        WidenInspector
    ]
);
