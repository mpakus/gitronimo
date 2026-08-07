# Implementation work log

## 2026-08-06 — Phase 0 / issue 1: initialize workspace and CI

**Intent:** create the reproducible Rust workspace and quality gates required before the GPUI window spike.

**Files:** root Cargo configuration, minimal boundary crates, policy documents, ADR template, and CI workflows.

**Acceptance checks:** workspace metadata resolves; exact GPUI dependencies resolve from `ui_kit`; formatting, Clippy, tests, and cargo-deny have an executable local/CI command.

**Deferred:** GPUI window, menus, components, resizable panels, packaging, and Git process behavior are separate checklist items.

**Environment blocker:** the local GPUI build currently stops in its Metal shader build step because Xcode's Metal Toolchain is absent. Install it with `xcodebuild -downloadComponent MetalToolchain` before running workspace-wide compile, Clippy, or tests. This is a machine prerequisite, not a dependency or source failure.

**Resolved environment prerequisite:** Metal Toolchain 17F109 was installed. The workspace now builds and its full Clippy/test gates pass. `cargo-deny` is installed by CI but is not available as a local Cargo subcommand yet, so its local result remains pending. The lockfile is generated and ready for the initial commit; its checklist item stays open until that commit is made.

## 2026-08-06 — Phase 0 / issue 2: minimal GPUI macOS window

**Intent:** prove the pinned GPUI release opens a native application window with a standard title bar and an enforced minimum size.

**Files:** `apps/desktop/src/main.rs` and its manifest.

**Acceptance checks:** the application opens a centered 1200×800 window titled “Gitronimo”, enforces an 800×560 minimum size, and a `#[gpui::test]` opens the view on GPUI's test platform.

**Deferred:** menus, actions, panels, themes, and background tasks are separate Phase 0 checklist items.

**Verification:** the `#[gpui::test]` passed, and `cargo run -p gitronimo-desktop` launched the native process successfully before it was stopped. No background task is created by this spike.

## 2026-08-06 — Phase 0 / issue 3: actions, keybindings, and application menu

**Intent:** establish typed global actions and prove macOS menus and keyboard shortcuts can dispatch them without coupling to repository behavior.

**Files:** desktop actions, menu, and application composition modules.

**Acceptance checks:** an application menu exposes File and View actions, Command-O and Command-R dispatch to the root view, and a GPUI test verifies the action event.

**Verification:** two GPUI tests pass: one creates the native-window view and one dispatches Command-O and Command-R through the keymap to the root view. The File and View menus use those same typed actions.

## 2026-08-06 — Phase 0 / issue 4: design tokens and UI boundary

**Intent:** create the project-owned semantic color vocabulary and remove raw desktop-view colors.

**Files:** `ui_kit` theme module and desktop root view.

**Acceptance checks:** every required semantic Git-client color is exposed by `ThemeColors`; the desktop imports it from `ui_kit`; and a unit test proves the window and panel backgrounds remain distinct.

**Deferred:** component wrappers, typography scale, spacing, radii, shadows, and appearance switching wait for their active checklist items.

**Verification:** `Theme::dark()` supplies the original semantic palette, the desktop no longer contains raw color values, and the focused `ui_kit`/desktop test suite passes.

## 2026-08-06 — Phase 0 / issue 5: component-library evaluation

**Intent:** evaluate the exact `gpui-component 0.5.1` release without letting it leak into product views.

**Files:** workspace dependency configuration and ADR 0001.

**Acceptance checks:** the exact `gpui 0.2.2`/`gpui-component 0.5.1` pair compiles; no desktop or domain code imports the component crate; an explicit, reversible decision is recorded.

**Decision:** remove `gpui-component`. Its required global initializer owns theme and broad application state, which conflicts with Gitronimo's original token system and a narrow `ui_kit` boundary. Build only the needed controls with GPUI primitives.

## 2026-08-06 — Phase 0 / issue 7: virtualized synthetic history

**Intent:** prove the GPUI workspace can display 100,000 history rows without constructing every row.

**Files:** desktop root view.

**Acceptance checks:** `uniform_list` receives 100,000 entries and its processor constructs only the visible range; the app renders an original sidebar, history, and inspector proof layout.

**Verification:** the focused GPUI tests and workspace Clippy gate pass with the 100,000-row `uniform_list` in place.

## 2026-08-06 — Phase 0 / issue 8: graph canvas

**Intent:** prove a custom GPUI canvas can paint graph lanes in virtualized history rows.

**Files:** desktop root view.

**Acceptance checks:** each visible synthetic-history row contains a canvas that paints a lane segment from the project graph palette; no paths are created for off-screen rows.

**Verification:** focused desktop tests and workspace Clippy pass with the custom canvas in the `uniform_list` processor.

## 2026-08-06 — Phase 0 / theme switch

**Intent:** prove project-owned light and dark appearances can switch without view-level raw colors.

**Verification:** `Theme::for_appearance` supplies both semantic palettes; Command-Shift-L and the View menu dispatch the toggle action; focused tests and Clippy pass.

## 2026-08-06 — Phase 0 / resizable panes

**Intent:** prove the three-pane layout can resize through keyboard-accessible actions.

**Verification:** Command-Option-Left and Command-Option-Right widen the sidebar and inspector within safe bounds; the bounds test and focused GPUI suite pass.

## 2026-08-06 — Phase 0 / Git executable and process spike

**Intent:** validate Finder-safe Git discovery, typed process invocation, byte-preserving status/history parsing, local fetch execution, and cancellation against real temporary repositories.

**Files:** `crates/git_cli/src/lib.rs`, its manifest, and this work log.

**Acceptance checks:** discovery considers a configured executable, GUI `PATH`, and standard macOS paths; every process uses `std::process::Command` argument APIs; `git --version`, NUL-delimited porcelain-v2 status, 500 explicit-separator history records, local fetch, and cancelling an active Git process are covered by tests.

**Deferred:** repository session state, mutation coordination, product status models, and UI integration remain later Phase 1+ work.

**Verification:** four real-repository tests pass with the installed Git executable: configured/GUI-safe/common-path discovery and `--version`; NUL-delimited porcelain-v2 status preserving a tab, newline, space, and Unicode filename; 500 NUL-separated log records; and a local bare-remote fetch whose stderr stream is read before its process completes. A piped `git hash-object --stdin` is cancelled through `Child::kill`. All application calls use `std::process::Command::args`, never a shell string.

## 2026-08-06 — Phase 0 / dependency-policy correction

**Intent:** make the committed `cargo-deny` policy accurately validate the pinned GPUI dependency graph and internal workspace dependencies.

**Files:** `deny.toml`, workspace manifests, and dependency policy documentation.

**Acceptance checks:** `cargo deny check` permits the explicitly reviewed OSI-compatible transitive licenses required by GPUI and accepts versioned internal path dependencies without weakening third-party wildcard checks.

## 2026-08-06 — Phase 0 / macOS packaging spike

**Intent:** package the desktop binary as an original, unsigned development `.app`, document the signing handoff, and make CI retain an unsigned artifact.

**Files:** `Packager.toml`, original icon assets, packaging documentation, CI workflow, and this work log.

**Acceptance checks:** pinned `cargo-packager 0.11.8` creates an app bundle with a unique bundle identifier and icon; the bundle launches outside Cargo; Apple Silicon and Intel builds compile; signing is documented without committing credentials; CI uploads the unsigned development artifact.

**Verification:** `cargo-packager 0.11.8` built `target/release/Gitronimo.app`. Its `Info.plist` declares `com.gitronimo.desktop`, `public.app-category.developer-tools`, and the original `gitronimo.icns`; `open target/release/Gitronimo.app` launched its embedded executable directly. The host is Apple Silicon (`arm64`); the installed `x86_64-apple-darwin` target also produces a Mach-O x86_64 release executable. CI installs the same exact packager version and uploads the unsigned bundle. The local signing handoff names no credentials.

## 2026-08-06 — Phase 0 / GPUI lifecycle check

**Intent:** verify that closing the prototype does not leave a desktop process or child task alive.

**Files:** `PLAN.md` and this work log only.

**Acceptance checks:** terminate the launched app, confirm no Gitronimo executable remains, and relaunch the packaged app. The prototype deliberately creates no GPUI background task or Git process.

**Verification:** after terminating the launched packaged executable, `pgrep -fl gitronimo-desktop` produced no output. The prototype has no spawned GPUI task, worker, or Git child process, so no background task remained to leak.

## 2026-08-06 — Phase 0 / decision-rule alignment

**Intent:** align `AGENTS.md` with ADR 0001 after the component-library evaluation.

**Files:** `AGENTS.md` and this work log.

**Acceptance checks:** future changes cannot treat the rejected component library as an adopted dependency without replacing the documented decision.

**Verification:** `git_domain`, `app_core`, `git_cli`, `test_support`, and `ui_kit` are separate workspace crates, with GPUI confined to the desktop/UI boundary.

## 2026-08-06 — Phase 0 / clean-source verification

**Intent:** prove the current source compiles outside the active, dirty worktree without copying the user-provided Tower screenshots.

**Files:** `PLAN.md` and this work log only.

**Acceptance checks:** a fresh temporary source copy, excluding `.git`, build outputs, machine metadata, and `docs/screens`, passes the workspace build and test commands.

**Verification:** a fresh `/tmp` source copy compiled successfully, then passed all workspace unit and doc tests with Cargo, Rustc, and Rustdoc explicitly pinned to the repository's Rust 1.97.1 toolchain. The copy deliberately excluded `docs/screens`; it is source-isolation evidence, not a substitute for committing the current worktree and testing that exact checkout.

## 2026-08-06 — Phase 0 / commit-backed verification

**Intent:** commit the implementation files required by Phase 0 without including user-provided Tower screenshots or unrelated machine metadata, then validate the exact commit in a detached clean worktree.

**Files:** all Phase 0 source, policy, packaging, and original icon files; explicitly excluding `docs/screens` and `.DS_Store` files.

**Acceptance checks:** the commit contains `Cargo.lock` and every required Phase 0 implementation file, and its detached worktree passes the workspace build and test suite.

**Verification:** commit `65998ae` contains the Phase 0 implementation and `Cargo.lock`, without the Tower screenshots or `.DS_Store` files. A detached worktree at that exact commit passed `cargo build --workspace --all-targets` and the full workspace unit and doc test suite under the pinned Rust 1.97.1 Cargo/Rustc/Rustdoc executables.
