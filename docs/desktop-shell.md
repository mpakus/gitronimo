# Desktop shell chrome

Functional reference for the repository window chrome that sits outside individual views (toolbar, sidebar BRANCHES, activity bar, overlays).

## Layout

```
┌─ Toolbar (Fetch / Pull / Push / Sync · stash · Refresh · search · palette) ─┐
│ Sidebar │ Content (+ optional list/detail panes)                            │
│ BRANCHES│                                                                   │
│ (pins…) │                                                                   │
├─ Activity bar: [history] [network progress…] status line ───────────────────┤
└─ Overlays: palette, prompts, ref/commit menus, Pull/Push, confirms, About, App Settings ─┘
```

Preferences live at `~/Library/Application Support/Gitronimo/recent-repositories.json` (schema v1). Repository Settings opt-ins default **off** except as noted:

| Setting | Where | Behavior |
|---------|-------|----------|
| **Git engine** | Repository Settings | Default **gix**. **System Git** forces `git_cli`. Unmigrated workflows always use the installed executable. |
| **Auto-stash before switch and pull** | Repository Settings | Stash dirty work, run the switch or pull, reapply. Pull uses Git `--autostash`. Off by default. |
| **In-app updates** | **GitRonimo → Settings…** (Command-comma) | On by default. **Check now** (About **Check for updates**, GitRonimo menu **Check for Updates…**, or palette) can install a verified GitHub release zip. No check on launch. App Settings shows the installed `APP_VERSION`. |
| **AI commit messages** | Repository Settings | Composer **Suggest** / palette **Suggest commit message** POSTs the redacted staged diff to an OpenAI-compatible endpoint (HTTPS, or HTTP on `127.0.0.1` / `localhost` / `[::1]`). Endpoint and model persist in prefs; API key is Keychain `com.gitronimo.ai-commit`. Fills the composer only — never commits. Off by default. |

## Activity bar and message history

| Piece | Behavior |
|-------|----------|
| Status line | Current `activity` string; colored via `ActivityKind` (success / error / progress / confirm / info) |
| Clock / history button | Toggles **Message history** popup (bottom-left) |
| Popup | Newest-first, up to 100 entries; scrollable; colored dots + local `HH:MM` clock time |
| Network strip | Indeterminate progress + Cancel while a `NetworkOperation` runs |

All user-visible status text should go through `GitronimoApp::set_activity` so the history log stays complete. Consecutive identical lines are skipped. Consecutive working-copy refresh chatter (`Refreshing working copy…` / `Working copy refreshed…`) is **coalesced** so push completions, errors, and confirmations are not drowned out.

Palette entry: **Message history**.

## Confirm dialogs

Destructive or blocked Git outcomes use modal overlays (not only the activity flash):

| Flow | First step | Second step (if needed) |
|------|------------|-------------------------|
| Delete local branch | **Delete Branch** — Cancel / Delete (`git branch -d`) | If not fully merged → **Could Not Delete Branch** — Cancel / Delete (`git branch -D`) via `AppConfirmDialog::ForceDeleteBranch` |
| History → Hard reset | Choice prompt Soft / Mixed / Hard | **Hard Reset** confirm via `AppConfirmDialog::HardReset` |
| History → Revert / Delete | `AppConfirmDialog::RevertCommit` / `DropCommit` | — |
| Workflow → Finish topic | `AppConfirmDialog::FinishTopic` | merge/squash/rebase into configured bases, then delete the topic |
| GitRonimo Settings → Check now (updates on) | `AppConfirmDialog::InstallUpdate` | download zip, SHA-256, Gatekeeper, replace `.app` |
| Force push | Existing force-with-lease confirmation | — |

`AppConfirmDialog` is the shared enum for blocked-action confirmations; render through `workspace` modal helpers. Domain logic stays in `main.rs`, not in `Render`.

## Workflow

Toolbar **Workflow** opens the in-repo Workflow view when a repository is loaded, or the welcome Workflow hub otherwise. Palette: **Show workflow**.

- Templates: GitHub Flow, GitLab Flow, git-flow, plus Auto-detect from local branch names (`develop` → git-flow, `production`/`staging` → GitLab Flow, else GitHub Flow).
- Config persists per repository in preferences (`workflows` map, same path-locked RMW as pins).
- **Start…** prompts for a suffix and creates `{prefix}{name}` from the topic start point.
- **Finish…** confirms, then merge / squash / rebase-and-ff into each `merge_into` target and deletes the topic (force-delete after squash).
- **Sync** merges the topic’s start (or a child base’s parent) into HEAD.
- Deferred: Graphite CLI, git-flow CLI, stacked restack, auto-archive protection.

## History commit context menu

Right-click a History commit (`views/commit_context_menu.rs`) for a grouped menu (quoted short OID labels):

- Copy hash / info, Reveal in History, Check Out (detached), Create branch/tag, Save Patch, Export Files, Compare
- Reset / Revert / Rebase / Delete — enabled only when History is scoped to the current HEAD branch (`Current` or that branch’s named filter); otherwise disabled with a tooltip
- Amend and Edit Commit Message — HEAD commit only; **Edit** stays disabled (no interactive rebase-edit from History yet)

Chrome matches the sidebar ref menu (cursor-anchored overlay).

## Pinned and archived branches

- Pin / Unpin and Archive / Unarchive live on the local-branch context menu.
- State is per repository under `branch_organization` in preferences (`BranchOrganization { pinned, archived }`).
- **Pinned** branches render **flat at the top of BRANCHES** (bookmark icon, pin order). No separate “PINNED” section label.
- The checked-out branch shows a muted **HEAD** pill; when the branch is ahead or behind its upstream, the same pill includes `↑N` and/or `↓N` (toolbar tracking uses the same order).
- **Archived** branches move under an **ARCHIVED** section.
- Preference load-modify-save is serialized with a **path-keyed lock** so window-geometry / width writers cannot wipe pins (multiple `RecentRepositoryStore::new` handles share one lock per prefs path).

## Command palette (`Command-Shift-P`)

Searchable list (`PALETTE_COMMANDS` in `app_state.rs`); list viewport scrolls. Includes:

- Open repository, Fetch / Pull… / Push… / Sync, Refresh
- Stage all / Unstage all, Focus commit composer, Amend last commit, **Suggest commit message**
- Stashes: Save stash… / Save including untracked… (dialogs), Save stash snapshot…, Apply latest/selected… (apply dialog), Apply selected stash files, Branch / Pop / Drop selected…
- Create branch… / Create tag…
- Show working copy / history / stashes / remotes / settings / **workflow** / pull requests / branches review / reflog / LFS / worktrees / submodules / rebase / conflicts…
- Git LFS: status list, **Fetch Git LFS objects**, **Pull Git LFS objects** (system Git; cancellable; default remote)
- History filter, Reveal HEAD, selected-commit copy / checkout / reset / revert / patch / export / compare / new branch
- History tools (file history, blame, compare refs, browse tree, rebase onto, merge revision, squash/fixup/drop/reword, merge tool, signature check)
- Quick open file, Message history, Toggle appearance, Keyboard shortcuts, Navigate back/forward, **About GitRonimo**, **App settings**, **Check for updates**

Dispatch is `run_palette_command` in `main.rs`. Adding a user-facing action that already has a handler should usually add a `PaletteCommand` variant + label + match arm. Selected-commit actions require a History selection; Reset/Revert also require History scoped to the HEAD branch.

## Stashes (core)

- **Save stash** (toolbar Save, Stashes header, palette, `Command-Shift-S`): text prompt for message + Include untracked checkbox; optional pathspecs from Working Copy **Stash selected…**.
- **Save snapshot…** (Stashes header, palette): named stash that **keeps** the working copy. Include untracked is optional.
- **Apply** (toolbar Apply, Stashes Apply…): dialog with Delete after applying (pop) and Restore staging area (`--index`).
- Stashes detail: date + subject list; on select, changeset paths + read-only diff; **Apply selected files** / Apply… / Pop… / Drop… / Branch…. Click or Cmd-click files, then drag them onto **Working Copy** in the sidebar to restore those paths without dropping the stash.
- Create/apply/pop/drop/branch refresh Working Copy and the stash list. Settings can auto-stash before switch and pull (off by default).

## Submodules

Palette **Show submodules**. The view lists configured submodules and **Update all…** (`git submodule update --init` on system Git). There is no Finder-open action.

## Related docs

- Shortcuts: [keyboard-shortcuts.md](keyboard-shortcuts.md)
- UI patterns: [UI-IMPROVE.md](UI-IMPROVE.md)
- Architecture / prefs: [architecture.md](architecture.md)
- Recovery: [troubleshooting.md](troubleshooting.md)

## About GitRonimo

**GitRonimo → About GitRonimo** (also palette **About GitRonimo**) opens a centered black overlay: `assets/gitronimo-icon.png`, name, product version (`APP_VERSION` in `apps/desktop/src/views/about.rs` — currently **2.0.4**, bump after each release), “Made in Austin ✩ Texas”, https://aomega.co, in-app update status, and **Check for updates** (same handler as App Settings **Check now** and **GitRonimo → Check for Updates…**). Click outside to dismiss (fade). The macOS application menu title is the process/bundle name `GitRonimo`. The dock icon is `icon.png`.

Modal overlays (palette, text/choice prompts, Pull/Push, confirms, About, App Settings, message history, welcome +) fade in and out (~150ms). Sidebar ref/commit context menus stay instant. The Working Copy commit composer eases description and Amend/Sign-off open (~200ms height + fade) when the subject field is focused.

## App Settings

**GitRonimo → Settings…** (Command-comma, also palette **App settings**) opens a centered overlay with **UPDATES** (installed version, On/Off, **Check now**). In-app updates default on. Click outside to dismiss. Repository sidebar **Settings** keeps Git engine, auto-stash, AI commits, GitHub, and appearance.
