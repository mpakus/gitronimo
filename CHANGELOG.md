# Changelog

## Unreleased

## 2.0.4 — 2026-08-17

Product version **2.0.4** (About GitRonimo / `APP_VERSION` and packager bundle version). Tag `v2.0.4` for the notarized universal zip.

- Modal popups (About, App Settings, palette, prompts, Pull/Push, confirms, message history) fade in and out. Clicking Commit Subject eases the description and Amend/Sign-off fields open instead of jumping.
- About GitRonimo keeps the site link and **Check for updates** inside the card (the updates sentence no longer stretches the right column past 520px).
- Release publish uses GitHub REST (`gh api`) with retries instead of `gh release view` / `create`. Those GraphQL commands 503'd after v2.0.3 notarized (“No server is currently available”) while REST still listed releases.

## 2.0.3 — 2026-08-17

Product version **2.0.3** (About GitRonimo / `APP_VERSION` and packager bundle version). Tag `v2.0.3` for the notarized universal zip.

- In-app updates accept a `SHA256SUMS.txt` line that includes a directory prefix (the v2.0.2 release listed `target/release-universal/GitRonimo-v2.0.2.zip`). The release workflow now hashes the zip from its own directory so the filename is basename-only.

## 2.0.2 — 2026-08-17

Product version **2.0.2** (About GitRonimo / `APP_VERSION` and packager bundle version). Tag `v2.0.2` for the notarized universal zip.

- In-app updates default **on**. **GitRonimo → Settings…** (Command-comma) opens app Settings with the Updates toggle and **Check now**. Repository Settings no longer lists Updates. Off/On use unique control ids so the toggle actually persists.

## 2.0.1 — 2026-08-17

Product version **2.0.1** (About GitRonimo / `APP_VERSION` and packager bundle version). Tag `v2.0.1` for the notarized universal zip.

- Working Copy changes preview (Stage Chunk / Discard Chunk) scrolls vertically and horizontally when the hunk or a line is larger than the pane.

## 2.0.0 — 2026-08-16

Product version **2.0.0** (About GitRonimo / `APP_VERSION` and packager bundle version). Tag `v2.0.0` for the notarized universal zip.

- Default Git engine is gitoxide `gix` for repository discovery, HEAD, refs, working-copy status, history, tree/blob reads, unified diffs, stage/unstage/commit, and HTTPS fetch/clone. System Git remains the fallback and a Settings override (`Use system Git`); SSH/`file://` remotes, checkout/merge/rebase/stash/push, hooks, and signed commits stay on the installed Git. See [`docs/PLAN-v2.md`](docs/PLAN-v2.md) and ADR 0003.
- Git LFS view can fetch objects and pull them into the working copy (`git lfs fetch` / `git lfs pull` on system Git), with the same cancelable network progress as remotes.
- Stash files can be selected and dragged onto Working Copy (or applied with **Apply selected files**) to restore those paths without dropping the stash.
- Optional **Auto-stash before switch and pull** (Settings, off by default) stashes dirty work, runs the switch or pull, then reapplies the stash. Pull uses Git `--autostash`.
- **Save snapshot…** stores a named stash without resetting the working copy (`git stash create` + `store`, or push+apply when including untracked).
- Optional **In-app updates** (Settings, off by default). **Check for Updates…** (GitRonimo menu, About, Settings **Check now**, or palette) reads GitHub Releases, prompts, downloads the notarized zip, verifies SHA-256 and Gatekeeper (`codesign` + `spctl`), then replaces `GitRonimo.app`. No check on launch. No telemetry. Cargo/`target/` runs refuse install. About and Settings show the installed version.
- Optional **AI commit messages** (Settings, off by default). **Suggest** (composer or palette) sends only the redacted staged diff to a configured OpenAI-compatible endpoint (HTTPS, or HTTP on `127.0.0.1` / `localhost` / `[::1]`). The API key is Keychain `com.gitronimo.ai-commit`, not the GitHub PAT. The reply fills the composer; you still edit and click Commit. Failure leaves the composer unchanged.

## 1.0.0 — 2026-08-14

Product version **1.0.0** (About GitRonimo / `APP_VERSION` and packager bundle version). Daily Git client: working copy, history, branches, stashes, remotes, GitHub PRs (PAT), workflow templates. Tag `v1.0.0` for the notarized universal zip.

- Network progress bar follows Git `--progress` percentages when Git prints them (waiting bar until the first `%` line; no 92% cap).
- Commit-message temp files are `0o600` and always deleted; Git activity text redacts URL passwords and `ghp_` / `github_pat_` tokens.
- Drag a Working Copy file onto another macOS app (Zed, RubyMine, Finder, …) to open that path; a multi-selection drags every selected file that still exists.
- Message history timestamps are local `HH:MM` clock time, not relative ages.
- About / bundle version is **1.0.0**.

### Known limitations

- GitHub.com personal-access-token only (Settings). No OAuth device flow or enterprise/self-hosted GitHub endpoint.
- VoiceOver is limited by GPUI 0.2.2; see [troubleshooting](docs/troubleshooting.md#keyboard-and-assistive-technology).
- Reflog, Blame, Compare, Branches Review, LFS, Worktrees, Submodules, and Interactive rebase are command-palette destinations, not primary sidebar items.
- No in-app updater, localization, or AI commit-message assistance.
- Merge and rebase use the branch/commit menus, the Working Copy continue/abort banner, and the Interactive rebase view — not a separate step-by-step wizard.

## 0.9.2 — 2026-08-14

Product version **0.9.2** (About GitRonimo / `APP_VERSION` and packager bundle version).

- Dock/Finder icon from `icon.png` (packaged as `assets/gitronimo.icns`).
- About overlay shows `assets/gitronimo-icon.png`.
- README hero image is `docs/logo.png`.
- `./bin/build` packages a local unsigned `GitRonimo.app`.

## 0.9.1 — 2026-08-14

Product version **0.9.1** (About GitRonimo / `APP_VERSION` and packager bundle version). Signed, notarized **universal** macOS app (`GitRonimo-v0.9.1.zip`, arm64 + x86_64).

- GitHub release publishes one universal zip (Intel is the `x86_64` slice inside the same `.app`, not a second asset).
- Release workflow retries `notarytool --wait` on App Store Connect HTTP timeouts and uploads onto an existing tag release when present.
- App icon is `assets/gitronimo-icon.png` (packaged as `assets/gitronimo.icns`).

## 0.9 — 2026-08-13

Product version **0.9** (About GitRonimo / `APP_VERSION`). Unsigned Apple Silicon and Intel `.app` bundles; signed notarized distribution still requires the protected tag workflow.

### Shell and workflow

- Message history on the activity bar (scrollable; successes, errors, confirmations; working-copy refresh lines coalesced).
- Command palette expanded and scrollable: Fetch/Pull/Push/Sync, staging, stash, create branch/tag, amend, history selection actions (checkout/reset/revert/patch/export/compare), rebase onto, merge revision, settings, and secondary views.
- Branch delete Cancel/Delete; unmerged tips open **Could Not Delete Branch** for force delete (`git branch -D`).
- Pinned branches persist per repository and sort flat at the top of BRANCHES; preference writes are serialized so geometry saves cannot wipe pins.
- Network progress strip in the bottom activity bar during fetch/pull/push/sync.
- History commit context menu (right-click): copy, detached checkout, reset/revert/rebase with confirms, amend/reword on HEAD, branch/tag/patch/export/compare.
- Stashes core: save dialog (message + untracked + path-limited from WC), apply dialog (pop / restore index), stash changeset detail, branch-from-stash; `Command-Shift-S` opens Save stash.
- Current branch sidebar pill shows `HEAD ↑N` / `↓N` (same arrow order as the toolbar tracking line).
- Removed the Services welcome tab, in-repo destination, and palette command. GitHub account connect/sign-out lives in Settings.
- Workflow tab (welcome + in-repo): GitHub Flow / GitLab Flow / git-flow templates, auto-detect from local branches, Start / Finish / Sync topic branches; config persists per repository.
- Command-H hides GitRonimo (`Hide GitRonimo` in the application menu), matching Command-Q quit.
- Command-F focuses the toolbar search field (welcome repository filter or in-repo file search).
- macOS application menu is **GitRonimo** (binary / bundle name). **About GitRonimo** opens a black two-column overlay with `assets/gitronimo-icon.png`, product version **0.9** (`APP_VERSION` in `apps/desktop/src/views/about.rs`), “Made in Austin ✩ Texas”, and https://aomega.co.
- Crash-report panics: branch context-menu render no longer re-reads `GitronimoApp` (GPUI double-lease); pane resize / bookmark drag no longer copy a cursor style onto `on_drag` (GPUI `set_window_cursor_style` debug assert).

### Known limitations

- Merge and rebase wizards, OAuth/enterprise GitHub, and notarized distribution are not complete.
- Remote progress is partly indeterminate (no full Git stderr byte/object parsing everywhere).
- VoiceOver parity is limited by the pinned GPUI framework; see the [accessibility note](docs/troubleshooting.md#keyboard-and-assistive-technology).
- Secondary views (Reflog, Blame, Compare, Branches Review, etc.) remain reachable from the command palette rather than the primary sidebar.

## 0.1.0 beta

### Included

- Open local Git repositories and inspect working-copy status and diffs.
- Stage, unstage, discard tracked changes, and create commits (including amend and sign-off).
- Partial line and hunk staging in the diff viewer.
- Working Copy multi-select: Command-A, Shift-click, toggle deselect/reselect; batch stage/unstage via checkboxes on multi-selection.
- Browse bounded history with graph, scope filter, Changeset/Tree detail, and Commit Detail navigation.
- Stashes, remotes, pull requests, and settings surfaces (two-pane layouts).
- Welcome/Repositories: bookmarks, grouped recents, add/create/clone.
- Branch/ref context menu on sidebar right-click; trimmed workspace sidebar (Working Copy, History, Stashes, Settings).
- Fetch, pull, publish, and push with cancellation for network work.
- Pull and Push HEAD dialogs: remote branch/destination pickers, rebase option, push all tags, force-with-lease, recurse submodules, skip hooks.
- Recover safely from missing repositories, stale index locks, malformed preferences, and application panics.

### Safety and reliability

- Gitronimo uses your installed Git executable, credential helper, SSH configuration, hooks, signing, and filters.
- Force pushes use force-with-lease and require an explicit confirmation.
- Crash reports stay on your Mac and are never uploaded automatically.

### Known limitations

- Merge and rebase wizards, OAuth/enterprise GitHub, and notarized distribution are not complete.
- Remote progress is partly indeterminate (no full Git stderr byte/object parsing everywhere).
- VoiceOver parity is limited by the pinned GPUI framework; see the [accessibility note](docs/troubleshooting.md#keyboard-and-assistive-technology).
- Secondary views (Reflog, Blame, Compare, Branches Review, etc.) remain reachable from the command palette rather than the primary sidebar.
