//! Render modules that own the visual layout of each window region.
//!
//! Each module contains only `impl GitronimoApp` render methods and small
//! free render helpers. State-mutating behavior stays in `crate::main`; shared
//! types and helpers live in `crate::app_state`.

pub(crate) mod blame;
pub(crate) mod commit_composer;
pub(crate) mod commit_detail;
pub(crate) mod compare;
pub(crate) mod components;
pub(crate) mod conflicts;
pub(crate) mod diff_viewer;
pub(crate) mod file_history;
pub(crate) mod history;
pub(crate) mod lfs;
pub(crate) mod pull_requests;
pub(crate) mod rebase;
pub(crate) mod reflog;
pub(crate) mod remotes;
pub(crate) mod services;
pub(crate) mod sidebar;
pub(crate) mod stashes;
pub(crate) mod submodules;
pub(crate) mod toolbar;
pub(crate) mod tree;
pub(crate) mod welcome;
pub(crate) mod working_copy;
pub(crate) mod workspace;
pub(crate) mod worktrees;
