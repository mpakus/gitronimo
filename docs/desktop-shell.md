# Desktop shell chrome

Functional reference for the repository window chrome that sits outside individual views (toolbar, sidebar BRANCHES, activity bar, overlays). Updated for the 2026-08-12 shell pass.

## Layout

```
┌─ Toolbar (Fetch / Pull / Push / Sync · stash · Refresh · search · palette) ─┐
│ Sidebar │ Content (+ optional list/detail panes)                            │
│ BRANCHES│                                                                   │
│ (pins…) │                                                                   │
├─ Activity bar: [history] [network progress…] status line ───────────────────┤
└─ Overlays: palette, prompts, ref/commit menus, Pull/Push, confirms ─────────┘
```

Preferences live at `~/Library/Application Support/Gitronimo/recent-repositories.json` (schema v1).

## Activity bar and message history

| Piece | Behavior |
|-------|----------|
| Status line | Current `activity` string; colored via `ActivityKind` (success / error / progress / confirm / info) |
| Clock / history button | Toggles **Message history** popup (bottom-left) |
| Popup | Newest-first, up to 100 entries; scrollable; colored dots + relative ages |
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
| Force push | Existing force-with-lease confirmation | — |

`AppConfirmDialog` is the shared enum for blocked-action confirmations; render through `workspace` modal helpers. Domain logic stays in `main.rs`, not in `Render`.

## History commit context menu

Right-click a History commit (`views/commit_context_menu.rs`) for a Tower-grouped menu (quoted short OID labels):

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
- Stage all / Unstage all, Focus commit composer, Amend last commit
- Stashes: Save stash… / Save including untracked… (dialogs), Apply latest/selected… (apply dialog), Branch / Pop / Drop selected…
- Create branch… / Create tag…
- Show working copy / history / stashes / remotes / settings / branches review / reflog / LFS / services / worktrees / submodules / rebase / conflicts…
- History filter, Reveal HEAD, selected-commit copy / checkout / reset / revert / patch / export / compare / new branch
- History tools (file history, blame, compare refs, browse tree, rebase onto, merge revision, squash/fixup/drop/reword, merge tool, signature check)
- Quick open file, Message history, Toggle appearance, Keyboard shortcuts, Navigate back/forward

Dispatch is `run_palette_command` in `main.rs`. Adding a user-facing action that already has a handler should usually add a `PaletteCommand` variant + label + match arm. Selected-commit actions require a History selection; Reset/Revert also require History scoped to the HEAD branch.

## Stashes (Tower-style core)

- **Save stash** (toolbar Save, Stashes header, palette, `Command-Shift-S`): text prompt for message + Include untracked checkbox; optional pathspecs from Working Copy **Stash selected…**.
- **Apply** (toolbar Apply, Stashes Apply…): dialog with Delete after applying (pop) and Restore staging area (`--index`).
- Stashes detail: date + subject list; on select, changeset paths + read-only diff; Apply… / Pop… / Drop… / Branch….
- Create/apply/pop/drop/branch refresh Working Copy and the stash list. Auto-stash, Snapshots, and DnD partial apply are deferred.

## Related docs

- Shortcuts: [keyboard-shortcuts.md](keyboard-shortcuts.md)
- UI patterns: [UI-IMPROVE.md](UI-IMPROVE.md)
- Architecture / prefs: [architecture.md](architecture.md)
- Recovery: [troubleshooting.md](troubleshooting.md)
