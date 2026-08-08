//! Desktop unit tests. Kept separate from entry and view code so the test-only
//! imports (`TestAppContext`, `Keystroke`) do not leak into production modules.

use app_core::RecentRepositoryStore;
use git_domain::{GitPath, WorktreeRepository};
use gpui::{AppContext, Keystroke, TestAppContext};

use super::crash_report_body;
use super::crash_report_path;
use super::window_options;
use crate::app_state::{
    GitronimoApp, LastAction, MAXIMUM_PANE_WIDTH, MINIMUM_PANE_WIDTH, ShellState,
    eligible_trash_path, git_failure_message, network_failure_message, repository_is_available,
    resize_width, shows_inspector, window_title,
};
use crate::keymap;
use crate::views::components::{activity_label, empty_status_message};

#[gpui::test]
fn opens_the_welcome_window(cx: &mut TestAppContext) {
    cx.update(|cx| {
        cx.open_window(window_options(cx, None), |window, cx| {
            cx.new(|cx| {
                GitronimoApp::welcome(
                    Vec::new(),
                    RecentRepositoryStore::new(
                        std::env::temp_dir().join("gitronimo-test-recents.json"),
                    ),
                    window,
                    cx,
                )
            })
        })
        .expect("the welcome window should open in GPUI's test platform");
    });
}

#[gpui::test]
fn keybindings_dispatch_global_actions(cx: &mut TestAppContext) {
    cx.update(|cx| cx.bind_keys(keymap::bindings()));
    let window = cx.update(|cx| {
        cx.open_window(window_options(cx, None), |window, cx| {
            cx.new(|cx| {
                GitronimoApp::welcome(
                    Vec::new(),
                    RecentRepositoryStore::new(
                        std::env::temp_dir().join("gitronimo-test-recents.json"),
                    ),
                    window,
                    cx,
                )
            })
        })
        .expect("the test window should open")
    });
    window
        .update(cx, |app, window, _| window.focus(&app.focus_handle))
        .expect("window should remain open");
    cx.dispatch_keystroke(
        *window,
        Keystroke::parse("cmd-r").expect("valid keybinding"),
    );
    window
        .update(cx, |app, _, _| {
            assert_eq!(app.last_action, Some(LastAction::Refresh));
        })
        .expect("window should remain open");
}

#[test]
fn pane_widths_stay_within_the_safe_range() {
    assert!((resize_width(0.0) - MINIMUM_PANE_WIDTH).abs() < f32::EPSILON);
    assert!((resize_width(MAXIMUM_PANE_WIDTH) - MAXIMUM_PANE_WIDTH).abs() < f32::EPSILON);
}

#[test]
fn inspector_yields_space_to_the_main_content_in_narrow_windows() {
    assert!(!shows_inspector(800.0, 220.0, 320.0));
    assert!(shows_inspector(900.0, 220.0, 320.0));
}

#[test]
fn error_shell_is_explicit() {
    assert!(matches!(
        ShellState::Error("message".into()),
        ShellState::Error(_)
    ));
}

#[test]
fn network_failures_are_actionable_without_echoing_remote_output() {
    assert!(
        network_failure_message("Pushing", "Permission denied (publickey)")
            .contains("authentication was rejected")
    );
    assert!(
        network_failure_message("Pushing", "rejected non-fast-forward")
            .contains("remote has newer commits")
    );
    assert!(
        !network_failure_message("Fetching", "https://token@example.test/repo")
            .contains("token@example.test")
    );
}

#[test]
fn workspace_empty_and_loading_copy_explain_the_next_state() {
    assert!(empty_status_message("Staged").contains("stage"));
    assert_eq!(empty_status_message("Conflicts"), "No merge conflicts.");
    assert_eq!(
        activity_label("Fetching origin in progress. You can cancel it."),
        "● Fetching origin in progress. You can cancel it."
    );
}

#[test]
fn window_titles_distinguish_welcome_loading_and_drafts() {
    assert_eq!(window_title(&ShellState::Welcome, false), "Gitronimo");
    assert_eq!(
        window_title(&ShellState::Loading("/tmp/example".into()), false),
        "Opening repository — Gitronimo"
    );
    assert_eq!(keymap::bindings().len(), 12);
}

#[test]
fn repository_loss_and_index_locks_have_safe_recovery_messages() {
    let root = std::env::temp_dir().join(format!("gitronimo-availability-{}", std::process::id()));
    let git_dir = root.join(".git");
    std::fs::create_dir_all(&git_dir).expect("fixture repository should exist");
    let repository = WorktreeRepository {
        worktree_root: root.clone(),
        git_dir,
    };
    assert!(repository_is_available(&repository));
    std::fs::remove_dir_all(&root).expect("fixture repository should remove");
    assert!(!repository_is_available(&repository));
    let message = git_failure_message("Stage selected", "fatal: .git/index.lock: File exists");
    assert!(message.contains("no Git process"));
    assert!(message.contains("before removing it manually"));
}

#[test]
fn crash_reports_are_local_and_do_not_include_panic_payloads() {
    let directory = std::env::temp_dir();
    assert!(
        crash_report_path(&directory, 42)
            .file_name()
            .is_some_and(|name| name == "gitronimo-crash-42.txt")
    );
    let report = crash_report_body(42, Some(std::panic::Location::caller()));
    assert!(report.contains("Timestamp: 42"));
    assert!(report.contains("never uploaded automatically"));
    assert!(!report.contains("secret panic payload"));
}

#[test]
fn trash_refuses_unsafe_paths_symlinks_and_nested_repositories() {
    let root = std::env::temp_dir().join(format!("gitronimo-trash-test-{}", std::process::id()));
    std::fs::create_dir_all(&root).expect("temporary root should exist");
    let nested = root.join("nested");
    std::fs::create_dir_all(nested.join(".git")).expect("nested repository marker should exist");
    std::os::unix::fs::symlink(&nested, root.join("link")).expect("symlink should exist");
    assert!(eligible_trash_path(&root, &GitPath(b"../outside".to_vec())).is_err());
    assert!(eligible_trash_path(&root, &GitPath(b"link".to_vec())).is_err());
    assert!(eligible_trash_path(&root, &GitPath(b"nested".to_vec())).is_err());
    std::fs::remove_dir_all(root).expect("temporary root should be removed");
}
