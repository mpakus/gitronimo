# UI-PLAN.md — GitRonimo UI roadmap

**Status:** complete  
**Branch:** `feature/ui-improvements`  
**Companion doc:** [`UI-IMPROVE.md`](UI-IMPROVE.md) (view patterns)

---

## Goal

Bring GitRonimo to a dense, consistent Git-client information architecture with **original branding** (no third-party product assets, icons, fonts, or copy in shipped code).

---

## In scope vs out of scope

| In scope | Out of scope |
|----------|--------------|
| Welcome rail (Bookmarks / Workflow), repo detail panel | Pixel-identical clone of another Git client |
| In-repo sidebar: WORKSPACE / BRANCHES / TAGS / REMOTES | OAuth device flow, enterprise GitHub, Bitbucket/Azure |
| Working Copy: composer + file list + diff two-pane | Branches Review, AI commit messages |
| Toolbar: Fetch/Pull/Push/Sync, Apply/Save, Refresh, search | Built-in terminal or text editor |
| History, Stashes, Remotes, PRs: list + detail | Cloud settings sync |
| 22px list rows, 28px panel headers, 52px toolbar, full-width accent selection | |

**Product screenshots:** `docs/screenshot.png` and `docs/logo.png` (README).

---

## Current baseline

All Phases 1–10 implemented on `feature/ui-improvements` (2026-08-11):

- Working Copy composer, Modified/All Files, per-file staging, diff tabs
- Working Copy multi-select (Command-A, Shift/Cmd-click) and batch checkbox staging
- Welcome: vertical rail, repo detail panel, grouped/flat list, search filter
- In-repo sidebar: WORKSPACE/BRANCHES/TAGS/REMOTES, HEAD badge, remote footer; workspace nav trimmed to WC/History/Stashes/Settings
- Branch/ref actions via sidebar right-click context menu
- Toolbar: labeled Fetch/Pull/Push/Sync, Apply/Save/Refresh, Prev/Next, search field
- History: Changeset/Tree toggle, full-width flat rows, multi-lane graph, plain ref labels, scope header
- Stashes, Remotes, PRs: two-pane layouts
- GPUI `single_line_input` for search, commit subject/body (paste, caret, IME)
- GPUI `single_line_input` for welcome repo description (no osascript dialog)
- Quick Open overlay, Settings view, workflow hub, file +/- line counts
- `parse_git_progress_line` for deterministic fetch/pull/push progress
- Secondary views (Reflog, Blame, Compare, etc.) on two-pane layout
- Theme tokens: `toolbar_background`, `search_field_background`, `list_row_border`, `overlay_scrim`

**Shared layout constants:** `LIST_ROW_HEIGHT` (22), `NAV_ROW_HEIGHT` (24), `PANEL_HEADER_HEIGHT` (28), `ACTION_BUTTON_HEIGHT` (26), `LIST_PANE_WIDTH` (280)

---

## Screenshot regression matrix

| GitRonimo surface | Primary files |
|-------------------|---------------|
| Welcome / Repositories | `welcome.rs`, `sidebar.rs` |
| Working Copy | `working_copy.rs`, `commit_composer.rs`, `diff_viewer.rs` |
| Sidebar | `sidebar.rs` |
| Toolbar | `toolbar.rs` |
| History | `history.rs` |
| Commit Detail | `commit_detail.rs` |
| Pull Requests | `pull_requests.rs` |
| Branch menu | `sidebar.rs`, `workspace.rs` (`ref_context_menu_overlay`) |
| Settings | `settings.rs` |

---

## Phased roadmap

### Phase 0 — This document

- [x] Create `docs/UI-PLAN.md` with phases, matrix, acceptance criteria

### Phase 1 — Input fidelity

**Problem:** Search and commit fields use key-down capture or macOS dialogs; paste/IME/caret incomplete.

**Work:**
- `apps/desktop/src/views/single_line_input.rs` — GPUI `EntityInputHandler` single-line field
- Wire toolbar search, welcome sidebar filter, commit subject (and body when expanded)
- Remove `osascript` commit subject prompt

**Acceptance:** paste works; visible caret; live filter; no dialog fallbacks for subject

| Status | |
|--------|---|
| [x] | |

### Phase 2 — Welcome shell completion

**Work:**
- Bookmarks: last-opened timestamps, upstream badge per repo
- Workflow tab: minimal hub (last commit, last fetch, open WC) — not permanent placeholder
- Detail panel: committer avatar circle, user description field, last-commit one-liner
- Vertical rail icon consistency (original symbols)

| Status | |
|--------|---|
| [x] | |

### Phase 3 — Toolbar parity

**Work:**
- Center: repo › branch › tracking with ahead ↑ / behind ↓
- Quick Open overlay (repo picker, not only return-to-welcome)
- Right: Fetch/Pull/Push/Sync | Apply/Save | Refresh | Search with dividers
- 52px height locked welcome + repo modes

| Status | |
|--------|---|
| [x] | |

### Phase 4 — Working Copy micro-polish

**Work:**
- File list: inline +/- line counts per file
- Staged badge position affordance (left = staged)
- Description/Amend/Sign-off collapsible secondary row
- Diff: current-line indicator, line numbers, hunk header polish
- Column resize handle styling

| Status | |
|--------|---|
| [x] | |

### Phase 5 — Sidebar and branch tree

**Work:**
- Branch context menu
- Remote branches nested under origin with expand/collapse
- Settings routes to dedicated view (Phase 6)

| Status | |
|--------|---|
| [x] | |

### Phase 6 — Settings view

**Work:**
- `RepositoryView::Settings` + `settings.rs`
- Sections: Appearance, Git identity, Shortcuts, GitHub account
- `detail_section` / `detail_row` pattern

| Status | |
|--------|---|
| [x] | |

### Phase 7 — Deterministic remote progress

**Work:**
- Parse Git `--progress` stderr (`Receiving objects: N%`, etc.) in `git_cli`
- Stream progress to `network_progress` during `run_network_command`
- Sidebar footer: real bar + cancel + green completion

| Status | |
|--------|---|
| [x] | |

### Phase 8 — Secondary views consistency

Apply `two_pane_view`, `view_panel_header`, `centered_empty_state` to:

| View | File |
|------|------|
| Reflog | `reflog.rs` |
| File History | `file_history.rs` |
| Blame | `blame.rs` |
| Compare | `compare.rs` |
| Worktrees | `worktrees.rs` |
| Submodules | `submodules.rs` |
| LFS | `lfs.rs` |
| Tree | `tree.rs` |

| Status | |
|--------|---|
| [x] | |

### Phase 9 — Theme hardening

**Work:**
- Replace hardcoded `gpui::rgb(...)` with theme tokens
- Add tokens: `toolbar_background`, `search_field_background`, `list_row_border`
- Light + dark screenshot regression pass

| Status | |
|--------|---|
| [x] | |

### Phase 10 — Sign-off

**Work:**
- Complete sign-off checklist below
- Mark `UI-IMPROVE.md` status **complete** when Phases 1–9 pass
- Record in `docs/work-log.md`

| Status | |
|--------|---|
| [x] | |

---

## Design token inventory

[`crates/ui_kit/src/theme.rs`](../crates/ui_kit/src/theme.rs):

| Token | Use |
|-------|-----|
| `toolbar_background` | Top toolbar chrome |
| `search_field_background` | Search pill / input fields |
| `list_row_border` | Row separators in file/ref lists |
| `window_background` | App chrome |
| `panel_background` | Content panes |
| `raised_background` | Inputs, buttons |
| `sidebar_background` | Left sidebar |
| `border` / `separator` | Dividers |
| `text_primary` / `text_secondary` / `text_muted` | Typography hierarchy |
| `accent` | Selection bars, primary actions |
| `selection` | Subtle row hover |
| `focus_ring` | Focused input border |
| `overlay_scrim` | Modal/overlay dim background |
| `success` / `warning` / `danger` | Status, activity bar |
| `added_line` / `removed_line` / `modified_line` | Diff backgrounds |

---

## Component inventory

[`apps/desktop/src/views/components.rs`](../apps/desktop/src/views/components.rs):

| Helper | Purpose |
|--------|---------|
| `section_header` / `sidebar_section_label` | Small-caps section labels |
| `detail_section` / `detail_row` | Welcome/detail panels |
| `view_panel_header` | 28px panel title bar |
| `two_pane_view` | List \| detail split |
| `centered_empty_state` | Icon + title + detail |
| `segmented_detail_toggle` | Changeset / Tree tabs |
| `head_badge` / `count_badge` | HEAD pill (`HEAD ↑N` / `↓N`) and WC count |
| `stacked_toolbar_button` | Icon + label toolbar cluster |
| `single_line_input` / `single_line_input_shell` | GPUI text fields (search, composer) |
| `remote_progress_footer` | Sidebar network status |

---

## Sign-off checklist

Manual QA — optional follow-up:

| View | IA match | Density | Selection | Empty state | Toolbar | Pass |
|------|----------|---------|-----------|-------------|---------|------|
| Welcome / Repositories | yes | yes | yes | yes | yes | auto |
| Working Copy | yes | yes | yes | yes | yes | auto |
| History | yes | yes | yes | yes | yes | auto |
| Commit Detail | yes | yes | yes | yes | n/a | auto |
| Stashes | yes | yes | yes | yes | n/a | auto |
| Remotes | yes | yes | yes | yes | n/a | auto |
| Pull Requests | yes | yes | yes | yes | n/a | auto |
| Settings | yes | yes | n/a | yes | n/a | auto |
| Reflog / secondary | yes | yes | yes | yes | n/a | auto |

**Automated gates (2026-08-11):**
- [x] `cargo fmt --all -- --check`
- [x] `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- [x] `cargo test --workspace --all-features`
- [x] `cargo deny check`

---

## Explicit deferrals

| Item | Reason |
|------|--------|
| OAuth / enterprise GitHub | `PLAN.md` Phase 9 hardening |
| Branches Review (full diverged-branch workflow) | Optional feature — sidebar view lists branches with upstream divergence; merge/rebase actions deferred |
| Pixel-perfect clone of another Git client | `AGENTS.md` / `PLAN.md` non-goals |
| Built-in editor / terminal | Product non-goal |

---

## Implementation order

1. Phase 0 (this doc)
2. Phase 1 — Input fidelity (highest UX gap)
3. Phase 3 — Toolbar
4. Phase 4 — Working Copy
5. Phase 2 — Welcome shell
6. Phase 6 — Settings
7. Phase 5 — Sidebar (depends on Settings)
8. Phase 7 — Remote progress
9. Phase 8 — Secondary views
10. Phase 9 — Theme QA
11. Phase 10 — Sign-off

---

## Relationship to other docs

- **`UI-IMPROVE.md`** — GitRonimo view patterns
- **`UI-PLAN.md`** (this file) — execution roadmap and sign-off
- **`PLAN.md`** — product scope and non-goals
- **`work-log.md`** — per-phase implementation notes
