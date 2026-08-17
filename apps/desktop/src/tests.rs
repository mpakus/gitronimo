//! Desktop unit tests. Kept separate from entry and view code so the test-only
//! imports (`TestAppContext`, `Keystroke`) do not leak into production modules.

use app_core::RecentRepositoryStore;
use git_domain::{
    CommitIdentity, GitPath, InProgressOperation, ReflogEntry, WorktreeRepository, WorktreeStatus,
};
use gpui::{AppContext, Keystroke, TestAppContext};

use super::crash_report_body;
use super::crash_report_path;
use super::window_options;
use crate::app_state::{
    GitronimoApp, LastAction, MAXIMUM_PANE_WIDTH, MINIMUM_PANE_WIDTH, OperationAction, RefContext,
    RefContextSubmenu, RepositoryView, ShellState, eligible_trash_path, files_for_status_drag,
    git_failure_message, network_failure_message, repository_is_available, resize_width,
    window_title,
};
use crate::keymap;
use crate::menus;
use crate::views::commit_composer::commit_unavailable_reason;
use crate::views::components::{activity_label, format_divergence_arrows, head_badge_text};
use crate::views::working_copy::{
    WORKING_COPY_CLEAN_DETAIL, WORKING_COPY_CLEAN_TITLE, operation_conflict_overview,
};

#[test]
fn a_disabled_commit_button_names_what_is_missing() {
    assert_eq!(
        commit_unavailable_reason(false, true, 0, false),
        "Stage changes and write a commit subject"
    );
    assert_eq!(
        commit_unavailable_reason(false, false, 0, false),
        "Stage at least one change to commit"
    );
    assert_eq!(
        commit_unavailable_reason(false, true, 3, false),
        "Write a commit subject"
    );
    assert_eq!(
        commit_unavailable_reason(false, true, 0, true),
        "Write a commit subject",
        "amending needs a subject but no staged change"
    );
    assert_eq!(
        commit_unavailable_reason(true, false, 3, false),
        "Another Git operation is still running"
    );
}

#[test]
fn conflict_overview_names_the_count_and_the_next_step() {
    assert!(operation_conflict_overview(0).contains("No conflicted files"));
    assert!(operation_conflict_overview(3).contains("3 conflicted file(s)"));
    assert!(operation_conflict_overview(3).contains("Continue"));
}

#[gpui::test]
fn reflog_selection_moves_and_clamps_within_loaded_entries(cx: &mut TestAppContext) {
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
        .update(cx, |app, _, cx| {
            app.repository_view = RepositoryView::Reflog;
            app.reflog = vec![reflog_entry(b"bbbb"), reflog_entry(b"aaaa")];
            app.move_reflog_selection(1, cx);
            assert_eq!(app.selected_reflog, Some(1));
            app.move_reflog_selection(1, cx);
            assert_eq!(
                app.selected_reflog,
                Some(1),
                "selection should clamp at the newest entry"
            );
            app.move_reflog_selection(-3, cx);
            assert_eq!(
                app.selected_reflog,
                Some(0),
                "selection should clamp at the oldest entry"
            );
            app.move_reflog_selection(1, cx);
            assert_eq!(
                app.selected_reflog,
                Some(1),
                "selection should track the newest entry after clamping"
            );
        })
        .expect("window should remain open");
}

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
    assert_eq!(WORKING_COPY_CLEAN_TITLE, "Working tree clean");
    assert!(WORKING_COPY_CLEAN_DETAIL.contains("editor"));
    assert_eq!(
        activity_label("Fetching origin in progress. You can cancel it."),
        "● Fetching origin in progress. You can cancel it."
    );
    assert_eq!(
        activity_label(WORKING_COPY_CLEAN_TITLE),
        WORKING_COPY_CLEAN_TITLE
    );
}

#[test]
fn window_titles_distinguish_welcome_loading_and_drafts() {
    assert_eq!(window_title(&ShellState::Welcome, false), "Gitronimo");
    assert_eq!(
        window_title(&ShellState::Loading("/tmp/example".into()), false),
        "Opening repository — Gitronimo"
    );
    assert_eq!(keymap::bindings().len(), 17);
}

#[test]
fn divergence_arrows_use_up_then_down_order() {
    assert_eq!(format_divergence_arrows(0, 0), None);
    assert_eq!(format_divergence_arrows(1, 0).as_deref(), Some("\u{2191}1"));
    assert_eq!(format_divergence_arrows(0, 2).as_deref(), Some("\u{2193}2"));
    assert_eq!(
        format_divergence_arrows(1, 3).as_deref(),
        Some("\u{2191}1 \u{2193}3")
    );
    assert_eq!(head_badge_text(0, 0), "HEAD");
    assert_eq!(head_badge_text(1, 0), "HEAD \u{2191}1");
    assert_eq!(head_badge_text(1, 2), "HEAD \u{2191}1 \u{2193}2");
}

#[test]
fn command_q_is_bound_to_quit() {
    let quit = keymap::bindings()
        .into_iter()
        .find(|binding| binding.action().partial_eq(&crate::actions::Quit))
        .expect("Command-Q should be bound");
    let [keystroke] = quit.keystrokes() else {
        panic!("Quit should use a single keystroke");
    };
    assert_eq!(keystroke.key(), "q");
    assert!(keystroke.modifiers().platform, "Quit needs the Command key");
}

#[test]
fn command_h_is_bound_to_hide() {
    let hide = keymap::bindings()
        .into_iter()
        .find(|binding| binding.action().partial_eq(&crate::actions::Hide))
        .expect("Command-H should be bound");
    let [keystroke] = hide.keystrokes() else {
        panic!("Hide should use a single keystroke");
    };
    assert_eq!(keystroke.key(), "h");
    assert!(keystroke.modifiers().platform, "Hide needs the Command key");
}

#[test]
fn application_menu_is_named_gitronimo_and_starts_with_about() {
    let menus = menus::application_menus();
    assert_eq!(
        menus[0].name.as_ref(),
        "GitRonimo",
        "the application menu title must match the macOS process/bundle name"
    );
    let gpui::MenuItem::Action { name, .. } = &menus[0].items[0] else {
        panic!("the first application menu item should be About GitRonimo");
    };
    assert_eq!(name.as_ref(), "About GitRonimo");
    let gpui::MenuItem::Action { name, .. } = &menus[0].items[1] else {
        panic!("the second application menu item should be Check for Updates");
    };
    assert_eq!(name.as_ref(), "Check for Updates…");
}

#[test]
fn about_dialog_uses_the_release_version() {
    assert_eq!(
        crate::views::about::APP_VERSION,
        "2.0.1",
        "bump APP_VERSION in views/about.rs after each release"
    );
}

#[test]
fn network_progress_fill_follows_git_percentages() {
    assert!((crate::views::components::network_progress_fill(0.0) - 0.12).abs() < f32::EPSILON);
    assert!((crate::views::components::network_progress_fill(0.45) - 0.45).abs() < f32::EPSILON);
    assert!((crate::views::components::network_progress_fill(1.0) - 1.0).abs() < f32::EPSILON);
}

#[test]
fn command_f_is_bound_to_focus_search() {
    let find = keymap::bindings()
        .into_iter()
        .find(|binding| binding.action().partial_eq(&crate::actions::FocusSearch))
        .expect("Command-F should be bound");
    let [keystroke] = find.keystrokes() else {
        panic!("FocusSearch should use a single keystroke");
    };
    assert_eq!(keystroke.key(), "f");
    assert!(
        keystroke.modifiers().platform,
        "FocusSearch needs the Command key"
    );
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
    let leaked = git_failure_message(
        "Push",
        "fatal: could not read https://octocat:s3cret@github.com/org/repo.git",
    );
    assert!(!leaked.contains("s3cret"));
    assert!(leaked.contains("***:***@github.com"));
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

#[test]
fn status_file_drag_uses_existing_worktree_paths_and_multi_selection() {
    let root =
        std::env::temp_dir().join(format!("gitronimo-file-drag-test-{}", std::process::id()));
    std::fs::create_dir_all(&root).expect("temporary root should exist");
    std::fs::write(root.join("kept.txt"), b"ok").expect("kept file should exist");
    std::fs::write(root.join("also.txt"), b"ok").expect("second file should exist");
    let kept = GitPath(b"kept.txt".to_vec());
    let also = GitPath(b"also.txt".to_vec());
    let missing = GitPath(b"gone.txt".to_vec());
    let escaped = GitPath(b"../outside".to_vec());

    assert_eq!(
        files_for_status_drag(&root, &kept, &[]),
        vec![root.join("kept.txt")]
    );
    assert_eq!(
        files_for_status_drag(&root, &kept, std::slice::from_ref(&also)),
        vec![root.join("kept.txt")],
        "an unselected row drags only itself"
    );
    assert_eq!(
        files_for_status_drag(&root, &kept, &[kept.clone(), also.clone()]),
        vec![root.join("kept.txt"), root.join("also.txt")]
    );
    assert!(files_for_status_drag(&root, &missing, &[]).is_empty());
    assert!(files_for_status_drag(&root, &escaped, &[]).is_empty());
    std::fs::remove_dir_all(root).expect("temporary root should be removed");
}

fn reflog_entry(new_oid: &[u8]) -> ReflogEntry {
    ReflogEntry {
        old_oid: None,
        new_oid: new_oid.to_vec(),
        selector: "HEAD@{0}".into(),
        identity: CommitIdentity {
            name: b"Test".to_vec(),
            email: b"test@example.test".to_vec(),
            timestamp: 1,
        },
        subject: "test entry".into(),
    }
}

fn diff_line(kind: git_domain::DiffLineKind) -> git_domain::DiffLine {
    git_domain::DiffLine {
        kind,
        content: b"line".to_vec(),
        missing_final_newline: false,
        old_line: None,
        new_line: None,
    }
}

fn sample_loaded_diff() -> git_cli::LoadedDiff {
    git_cli::LoadedDiff {
        diff: git_domain::UnifiedDiff {
            files: vec![git_domain::DiffFile {
                hunks: vec![git_domain::DiffHunk {
                    header: b"@@ -1,3 +1,3 @@".to_vec(),
                    lines: vec![
                        diff_line(git_domain::DiffLineKind::Context),
                        diff_line(git_domain::DiffLineKind::Addition),
                        diff_line(git_domain::DiffLineKind::Removal),
                    ],
                }],
                ..Default::default()
            }],
        },
        truncated: false,
    }
}

#[gpui::test]
fn line_selection_toggles_only_change_lines_on_unstaged_text_diffs(cx: &mut TestAppContext) {
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
        .update(cx, |app, _, cx| {
            app.loaded_diff = Some(sample_loaded_diff());
            app.selected_diff = Some((GitPath(b"notes.txt".to_vec()), false));
            app.toggle_diff_line(0, 0, cx);
            assert!(
                app.selected_diff_lines.is_empty(),
                "context lines are not selectable"
            );
            app.toggle_diff_line(0, 1, cx);
            assert_eq!(app.selected_diff_lines, vec![(0, 1)]);
            app.toggle_diff_line(0, 2, cx);
            assert_eq!(app.selected_diff_lines, vec![(0, 1), (0, 2)]);
            app.toggle_diff_line(0, 1, cx);
            assert_eq!(app.selected_diff_lines, vec![(0, 2)]);
            app.selected_diff = Some((GitPath(b"notes.txt".to_vec()), true));
            app.toggle_diff_line(0, 2, cx);
            assert_eq!(
                app.selected_diff_lines,
                vec![(0, 2)],
                "staged diffs refuse line selection"
            );
        })
        .expect("window should remain open");
}

#[gpui::test]
fn line_discard_requires_confirmation_and_cancellation_is_a_no_op(cx: &mut TestAppContext) {
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
        .update(cx, |app, _, cx| {
            app.loaded_diff = Some(sample_loaded_diff());
            app.selected_diff = Some((GitPath(b"notes.txt".to_vec()), false));
            app.selected_diff_lines = vec![(0, 1)];
            app.request_line_discard(cx);
            assert!(app.pending_line_discard.is_some());
            app.cancel_line_discard(cx);
            assert!(app.pending_line_discard.is_none());
            assert_eq!(app.selected_diff_lines, vec![(0, 1)]);
        })
        .expect("window should remain open");
}

#[gpui::test]
fn hunk_discard_requires_confirmation_and_cancellation_is_a_no_op(cx: &mut TestAppContext) {
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
        .update(cx, |app, _, cx| {
            app.loaded_diff = Some(sample_loaded_diff());
            app.selected_diff = Some((GitPath(b"notes.txt".to_vec()), false));
            app.request_hunk_discard(1, cx);
            assert_eq!(
                app.pending_hunk_discard,
                Some((GitPath(b"notes.txt".to_vec()), 1))
            );
            app.cancel_hunk_discard(cx);
            assert!(app.pending_hunk_discard.is_none());
            app.selected_diff = Some((GitPath(b"notes.txt".to_vec()), true));
            app.request_hunk_discard(0, cx);
            assert!(
                app.pending_hunk_discard.is_none(),
                "staged diffs refuse hunk discard"
            );
        })
        .expect("window should remain open");
}

#[gpui::test]
fn operation_actions_require_a_paused_operation_and_cancel_is_a_no_op(cx: &mut TestAppContext) {
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
        .update(cx, |app, _, cx| {
            app.working_copy = Some(WorktreeStatus {
                operation: InProgressOperation::Rebase,
                ..Default::default()
            });
            app.request_operation_abort(cx);
            assert_eq!(app.pending_operation_action, Some(OperationAction::Abort));
            app.cancel_operation_action(cx);
            assert!(app.pending_operation_action.is_none());
            app.request_operation_continue(cx);
            assert_eq!(
                app.pending_operation_action,
                Some(OperationAction::Continue)
            );
            app.cancel_operation_action(cx);
            assert!(app.pending_operation_action.is_none());

            app.working_copy = Some(WorktreeStatus::default());
            app.request_operation_abort(cx);
            assert!(
                app.pending_operation_action.is_none(),
                "no paused operation means no request is recorded"
            );
        })
        .expect("window should remain open");
}

struct StagingFixture {
    repository: WorktreeRepository,
}

impl StagingFixture {
    fn new(name: &str) -> Self {
        let root = std::env::temp_dir().join(format!("gitronimo-staging-{name}"));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("fixture directory should be creatable");
        let root = std::fs::canonicalize(&root).expect("fixture root should resolve");
        let fixture = Self {
            repository: WorktreeRepository {
                git_dir: root.join(".git"),
                worktree_root: root,
            },
        };
        fixture.git(&["init", "--initial-branch=main"]);
        fixture.git(&["config", "user.email", "test@gitronimo.invalid"]);
        fixture.git(&["config", "user.name", "Gitronimo Test"]);
        for file in ["a.txt", "b.txt"] {
            fixture.write(file, "one\n");
        }
        fixture.git(&["add", "."]);
        fixture.git(&["commit", "-m", "seed"]);
        for file in ["a.txt", "b.txt"] {
            fixture.write(file, "two\n");
        }
        fixture
    }

    fn write(&self, name: &str, contents: &str) {
        std::fs::write(self.repository.worktree_root.join(name), contents)
            .expect("fixture file should be writable");
    }

    fn git(&self, args: &[&str]) -> String {
        let output = std::process::Command::new("git")
            .args(args)
            .current_dir(&self.repository.worktree_root)
            .output()
            .expect("git should run");
        assert!(output.status.success(), "git {args:?} failed: {output:?}");
        String::from_utf8(output.stdout).expect("git output should be utf-8")
    }

    fn staged_paths(&self) -> Vec<String> {
        self.git(&["diff", "--cached", "--name-only"])
            .lines()
            .map(str::to_owned)
            .collect()
    }

    fn status(&self) -> WorktreeStatus {
        git_cli::GitExecutable::discover()
            .expect("git should be discoverable")
            .worktree_status(&self.repository, false)
            .expect("status should parse")
    }
}

impl Drop for StagingFixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.repository.worktree_root);
    }
}

#[gpui::test]
fn checkbox_click_stages_the_whole_selection_then_unstages_it(cx: &mut TestAppContext) {
    let fixture = StagingFixture::new("selection");
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
    let a = GitPath(b"a.txt".to_vec());
    let b = GitPath(b"b.txt".to_vec());

    window
        .update(cx, |app, _, cx| {
            app.state = ShellState::Repository(fixture.repository.clone());
            app.working_copy = Some(fixture.status());
            app.selected_paths = vec![a.clone(), b.clone()];
            app.toggle_path_staged(&a, false, cx);
        })
        .expect("window should remain open");
    cx.run_until_parked();
    assert_eq!(
        fixture.staged_paths(),
        vec!["a.txt".to_owned(), "b.txt".to_owned()],
        "one checkbox click stages every selected file"
    );

    window
        .update(cx, |app, _, cx| {
            app.working_copy = Some(fixture.status());
            assert_eq!(
                app.selected_paths,
                vec![a.clone(), b.clone()],
                "the selection survives the mutation"
            );
            app.toggle_path_staged(&a, true, cx);
        })
        .expect("window should remain open");
    cx.run_until_parked();
    assert!(
        fixture.staged_paths().is_empty(),
        "clicking again clears every checkbox in the selection"
    );
}

#[gpui::test]
fn a_plain_click_still_selects_one_file_after_selecting_all(cx: &mut TestAppContext) {
    let fixture = StagingFixture::new("selection-click");
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
    let a = GitPath(b"a.txt".to_vec());
    let b = GitPath(b"b.txt".to_vec());

    window
        .update(cx, |app, _, cx| {
            app.state = ShellState::Repository(fixture.repository.clone());
            app.working_copy = Some(fixture.status());
            app.selected_paths = app.visible_status_paths();
            assert_eq!(app.selected_paths.len(), 2, "select all covers both files");

            app.select_status_path(a.clone(), false, false, false, cx);
            assert!(
                app.selected_paths.is_empty(),
                "clicking a row while everything is selected clears the selection"
            );

            app.select_status_path(b.clone(), false, false, false, cx);
            assert_eq!(
                app.selected_paths,
                vec![b.clone()],
                "clicking another row selects just that file"
            );

            app.selected_paths = app.visible_status_paths();
            app.select_status_path(a.clone(), false, false, false, cx);
            app.select_status_path(a.clone(), false, false, false, cx);
            assert_eq!(
                app.selected_paths.len(),
                2,
                "clicking the same row again restores the full selection"
            );
        })
        .expect("window should remain open");
}

#[gpui::test]
fn clicking_a_rendered_checkbox_stages_the_selection(cx: &mut TestAppContext) {
    let fixture = StagingFixture::new("rendered-click");
    let store =
        RecentRepositoryStore::new(std::env::temp_dir().join("gitronimo-test-recents.json"));
    let (app, cx) =
        cx.add_window_view(|window, cx| GitronimoApp::welcome(Vec::new(), store, window, cx));
    app.update(cx, |app, cx| {
        app.state = ShellState::Repository(fixture.repository.clone());
        app.working_copy = Some(fixture.status());
        app.selected_paths = app.visible_status_paths();
        cx.notify();
    });
    cx.run_until_parked();
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    let bounds = cx
        .debug_bounds("checkbox:a.txt")
        .expect("the checkbox for a.txt should be rendered");
    // Deliberately off-centre: the click target has to be forgiving of near misses.
    let near_miss = bounds.origin + gpui::point(gpui::px(2.0), gpui::px(2.0));
    cx.simulate_click(near_miss, gpui::Modifiers::none());
    cx.run_until_parked();

    assert_eq!(
        fixture.staged_paths(),
        vec!["a.txt".to_owned(), "b.txt".to_owned()],
        "clicking the rendered checkbox stages every selected file"
    );
    app.update(cx, |app, _| {
        assert_eq!(
            app.selected_paths.len(),
            2,
            "the click must not collapse the selection"
        );
    });
}

#[gpui::test]
fn navigation_history_does_not_add_an_inline_back_row(cx: &mut TestAppContext) {
    let fixture = StagingFixture::new("inline-back");
    let store =
        RecentRepositoryStore::new(std::env::temp_dir().join("gitronimo-test-recents.json"));
    let (app, cx) =
        cx.add_window_view(|window, cx| GitronimoApp::welcome(Vec::new(), store, window, cx));
    app.update(cx, |app, _| {
        app.state = ShellState::Repository(fixture.repository.clone());
        app.working_copy = Some(fixture.status());
        app.navigation_back.push(RepositoryView::History);
    });
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();

    assert!(
        cx.debug_bounds("button:Back").is_none(),
        "the toolbar chevrons own navigation; the content area must not add a Back row"
    );
}

#[gpui::test]
fn the_toolbar_pull_button_opens_the_pull_dialog(cx: &mut TestAppContext) {
    let fixture = StagingFixture::new("toolbar-pull");
    let store =
        RecentRepositoryStore::new(std::env::temp_dir().join("gitronimo-test-recents.json"));
    let (app, cx) =
        cx.add_window_view(|window, cx| GitronimoApp::welcome(Vec::new(), store, window, cx));
    app.update(cx, |app, _| {
        app.state = ShellState::Repository(fixture.repository.clone());
        app.working_copy = Some(fixture.status());
    });
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();

    let bounds = cx
        .debug_bounds("toolbar-button:Pull")
        .expect("the toolbar Pull button should be rendered");
    cx.simulate_click(bounds.center(), gpui::Modifiers::none());

    app.update(cx, |app, _| {
        assert!(
            app.pull_dialog.is_some(),
            "the toolbar Pull button opens the dialog"
        );
    });
}

#[gpui::test]
fn confirming_the_pull_dialog_starts_the_network_command(cx: &mut TestAppContext) {
    let fixture = StagingFixture::new("pull-dialog");
    let store =
        RecentRepositoryStore::new(std::env::temp_dir().join("gitronimo-test-recents.json"));
    let (app, cx) =
        cx.add_window_view(|window, cx| GitronimoApp::welcome(Vec::new(), store, window, cx));
    app.update(cx, |app, cx| {
        app.state = ShellState::Repository(fixture.repository.clone());
        app.working_copy = Some(fixture.status());
        app.open_pull_dialog(None, cx);
    });
    cx.run_until_parked();
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();

    let bounds = cx
        .debug_bounds("button:Pull")
        .expect("the Pull button should be rendered");
    cx.simulate_click(bounds.center(), gpui::Modifiers::none());

    app.update(cx, |app, _| {
        assert!(app.pull_dialog.is_none(), "confirming closes the dialog");
        assert!(
            app.mutation_in_flight || app.activity.contains("Pulling"),
            "confirming starts the pull command, activity was {:?}",
            app.activity
        );
    });
}

#[gpui::test]
fn checkbox_click_stages_a_single_file(cx: &mut TestAppContext) {
    let fixture = StagingFixture::new("single");
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
    let a = GitPath(b"a.txt".to_vec());

    window
        .update(cx, |app, _, cx| {
            app.state = ShellState::Repository(fixture.repository.clone());
            app.working_copy = Some(fixture.status());
            app.toggle_path_staged(&a, false, cx);
        })
        .expect("window should remain open");
    cx.run_until_parked();
    assert_eq!(
        fixture.staged_paths(),
        vec!["a.txt".to_owned()],
        "an unselected row stages only itself"
    );
}

#[gpui::test]
fn rendering_a_branch_context_menu_does_not_double_lease(cx: &mut TestAppContext) {
    let fixture = StagingFixture::new("ref-context-menu");
    let store =
        RecentRepositoryStore::new(std::env::temp_dir().join("gitronimo-test-recents.json"));
    let (app, cx) =
        cx.add_window_view(|window, cx| GitronimoApp::welcome(Vec::new(), store, window, cx));
    app.update(cx, |app, cx| {
        app.state = ShellState::Repository(fixture.repository.clone());
        app.working_copy = Some(fixture.status());
        app.open_ref_context_menu(RefContext::LocalBranch("main".into()), (40.0, 90.0), cx);
        app.open_ref_context_submenu(RefContextSubmenu::PushTo, cx);
    });
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    assert!(
        cx.debug_bounds("ref-context-menu").is_some(),
        "the branch menu must paint without re-reading GitronimoApp during Render"
    );
}

#[gpui::test]
fn sidebar_resize_handle_mouse_down_does_not_panic(cx: &mut TestAppContext) {
    let fixture = StagingFixture::new("resize-handle-click");
    let store =
        RecentRepositoryStore::new(std::env::temp_dir().join("gitronimo-test-recents.json"));
    let (app, cx) =
        cx.add_window_view(|window, cx| GitronimoApp::welcome(Vec::new(), store, window, cx));
    app.update(cx, |app, _| {
        app.state = ShellState::Repository(fixture.repository.clone());
        app.working_copy = Some(fixture.status());
    });
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    let bounds = cx
        .debug_bounds("sidebar-resize-handle")
        .expect("the sidebar resize handle should be rendered");
    cx.simulate_click(bounds.center(), gpui::Modifiers::none());
    cx.run_until_parked();
}

#[gpui::test]
fn about_overlay_renders_from_show_about_dialog(cx: &mut TestAppContext) {
    let store =
        RecentRepositoryStore::new(std::env::temp_dir().join("gitronimo-test-recents.json"));
    let (app, cx) =
        cx.add_window_view(|window, cx| GitronimoApp::welcome(Vec::new(), store, window, cx));
    app.update(cx, GitronimoApp::show_about_dialog);
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    assert!(
        cx.debug_bounds("about-gitronimo").is_some(),
        "About GitRonimo must paint the overlay"
    );
    assert!(
        cx.debug_bounds("about-check-updates").is_some(),
        "About GitRonimo must offer Check for updates"
    );
}

#[gpui::test]
fn working_copy_diff_preview_paints_a_scroll_area(cx: &mut TestAppContext) {
    let fixture = StagingFixture::new("diff-scroll");
    let store =
        RecentRepositoryStore::new(std::env::temp_dir().join("gitronimo-test-recents.json"));
    let (app, cx) =
        cx.add_window_view(|window, cx| GitronimoApp::welcome(Vec::new(), store, window, cx));
    app.update(cx, |app, _| {
        app.state = ShellState::Repository(fixture.repository.clone());
        app.working_copy = Some(fixture.status());
        let mut loaded = sample_loaded_diff();
        loaded.diff.files[0].hunks[0].lines[1].content = b"x".repeat(400);
        app.loaded_diff = Some(loaded);
        app.selected_diff = Some((GitPath(b"a.txt".to_vec()), false));
        app.repository_view = RepositoryView::WorkingCopy;
    });
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    let scroll = cx
        .debug_bounds("diff-scroll")
        .expect("the changes preview must be a scroll container");
    let content = cx
        .debug_bounds("diff-scroll-content")
        .expect("the changes preview must size content wider than long lines");
    assert!(
        content.size.width > scroll.size.width,
        "long diff lines must overflow the pane so they can scroll horizontally (content {:?}, pane {:?})",
        content.size.width,
        scroll.size.width
    );
    assert!(
        cx.debug_bounds("stage-chunk-0").is_some(),
        "Stage Chunk must stay on the hunk header"
    );
    assert!(
        cx.debug_bounds("discard-chunk-0").is_some(),
        "Discard Chunk must stay on the hunk header"
    );
}
