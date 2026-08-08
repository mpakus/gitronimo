//! Render modules that own the visual layout of each window region.
//!
//! Each module contains only `impl GitronimoApp` render methods and small
//! free render helpers. State-mutating behavior stays in `crate::main`; shared
//! types and helpers live in `crate::app_state`.

pub(crate) mod commit_composer;
pub(crate) mod components;
pub(crate) mod diff_viewer;
pub(crate) mod history;
pub(crate) mod sidebar;
pub(crate) mod toolbar;
pub(crate) mod welcome;
pub(crate) mod working_copy;
pub(crate) mod workspace;
