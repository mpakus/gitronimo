//! Branches Review — optional Tower-style view for comparing local branches.

use gpui::prelude::*;
use ui_kit::ThemeColors;

use crate::app_state::GitronimoApp;
use crate::views::components::{centered_empty_state, two_pane_view, view_panel_header};

impl GitronimoApp {
    #[allow(clippy::unused_self)]
    pub(crate) fn branches_review_view(
        &self,
        colors: &ThemeColors,
        _cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        two_pane_view(
            view_panel_header("Branches Review", colors, None),
            centered_empty_state(
                "No branches to review",
                "Local branches that diverge from their upstream will appear here. Fetch and push from the toolbar to keep tracking branches up to date.",
                colors,
            ),
            centered_empty_state(
                "Select a branch",
                "Choose a branch from the list to inspect its commits ahead of or behind upstream.",
                colors,
            ),
            colors,
        )
    }
}
