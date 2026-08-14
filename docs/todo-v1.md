# GitRonimo v1 todo

**Status:** living checklist  
**As of:** 2026-08-14 (product **0.9.2**)  
**Source:** `PLAN.md`, `CHANGELOG.md`, `README.md`, `docs/UI-*.md`, packaging / shell / architecture docs

This is the leftover work after the 0.9 series, not a rewrite of `PLAN.md`. Phases 0–8 in `PLAN.md` and UI Phases 1–10 in `docs/UI-PLAN.md` are already implemented. Do not reopen them unless a checkbox below names a specific gap.

Work one group at a time. Record intent in `docs/work-log.md` before coding.

---

## What’s already done (do not re-plan)

Daily Git client is in the app:

- Welcome / bookmarks, Working Copy (file + hunk/line staging), History + graph, stashes, remotes, PRs (GitHub PAT), Settings
- Fetch / pull / push with cancel; Pull/Push dialogs; force-with-lease behind confirm
- Merge / rebase / cherry-pick / revert / reset as **operations** (not guided wizards)
- Reflog, blame, file history, worktrees, submodules, LFS **status**, conflict overview, interactive rebase plan editor
- Command palette, message history, About GitRonimo, pins/archives, Workflow templates
- Universal signed/notarized GitHub release workflow (`v*` tags) — **v0.9.1** shipped; **0.9.2** is the in-tree product version

`PLAN.md` §3.1 MVP scope is functionally shipped. Unchecked boxes there are mostly **un-audited success criteria**, not missing features.

---

## 1. Doc truth (do first — cheap, unblocks everything)

The markdown set is the source of confusion. Several files still describe an earlier beta.

### 1.1 Broken or missing links

- [ ] Restore or replace `docs/screens/README.md`. It is deleted on disk (`docs/screens/gitronimo-*.png` gone) but still linked from `docs/README.md`, `AGENTS.md`, `docs/troubleshooting.md`, `docs/UI-PLAN.md`, `docs/UI-IMPROVE.md`. README now uses `docs/screenshot.png` + `docs/logo.png` instead.
- [ ] Decide the screenshot policy: committed product shots only (`docs/screenshot.png`), or a rebuilt `docs/screens/` inventory. Update every link to match.
- [ ] Stop pointing `README.md` at `GitRonimo-v0.9.2.zip` until that tag exists. Latest published zip is **v0.9.1**. Either ship 0.9.2 or say “latest release”.

### 1.2 Stale claims

- [ ] `CHANGELOG.md` 0.9 / 0.1.0 “Known limitations” still say **notarized distribution is not complete**. That is false after v0.9.1. Keep OAuth, wizards, VoiceOver, progress parsing.
- [ ] `docs/UI-IMPROVE.md` “Remaining gaps” still lists **Settings** and **IME-rich search** as unfinished. Settings exists; `single_line_input` shipped in UI-PLAN Phase 1. Replace with real leftovers (truncation, OAuth, progress).
- [ ] `docs/UI-PLAN.md` sign-off still cites **102 tests** (2026-08-11). Refresh the count or drop the number.
- [ ] `PLAN.md` §5.1 still presents `gpui-component = "=0.5.1"` as the baseline. ADR 0001 **removed** it. Point at `ui_kit` + ADR 0001.
- [ ] `PLAN.md` §24 suggested `AGENTS.md` still says “keep gpui-component inside ui_kit”. Align with current `AGENTS.md`.
- [ ] `PLAN.md` §18 security checklist and §4 success criteria are almost all `[ ]` even where the code exists. Audit each item and check it, or move it to “needs evidence” below — do not leave a 2400-line blueprint looking like MVP never started.
- [ ] `PLAN.md` Phase 6 Release still unchecked for Developer ID / notary / staple / tag, despite `.github/workflows/release.yml` and the v0.9.1 artifact. Mark “workflow exists; per-release smoke still manual”.
- [ ] Dual versioning: Cargo workspace **0.1.0** vs product **0.9.2**. One short note at the top of `PLAN.md` and `README.md` (already in README) should also live in `PLAN.md` §27 so agents stop bumping the wrong number.
- [ ] `docs/work-log.md` is ~2.2k lines. Archive 2026-08-06…08-12 into `docs/work-log-archive.md` (or split by week). Keep the live log short.

### 1.3 Index and naming

- [ ] `AGENTS.md` documentation map still lists `docs/screens/README.md`. Add `docs/todo-v1.md`; drop or fix screens.
- [ ] Expand `TRADEMARKS.md` to cover GitHub and other marks used in-product. Copy must not imply affiliation.
- [ ] `docs/desktop-shell.md` header still says “Updated for the 2026-08-12 shell pass”. Date it to the current shell, or drop the date.

---

## 2. v1 product remaining

These are the real gaps named by `PLAN.md` post-MVP leftovers, `CHANGELOG` known limitations, and `UI-IMPROVE` / `desktop-shell` deferrals.

### 2.1 Must-have for calling it 1.0

- [ ] **Audit MVP success criteria** (`PLAN.md` §4) with evidence, not vibes:
  - porcelain-v2 edge cases (spaces, Unicode, tabs, newlines, renames, ignored, conflicts, submodules)
  - no shell-string Git (code review / grep)
  - one mutation at a time per repo
  - history stays usable at ~50k commits; first page is bounded
  - 10k-file working copy does not mount every row
  - staging / commit / fetch / pull / push integration tests exist (likely already; confirm)
  - destructive confirms name repo + paths
  - failures show command category + exit + redacted stderr
  - no third-party product assets in the `.app` bundle
- [ ] **Audit `PLAN.md` §18 security checklist** the same way (shell, helpers, no private-key reads, URL redaction, temp-file perms, `--` path sep, force-with-lease default, `cargo deny` + advisories in CI, checksums on the GitHub release).
- [ ] **Clean-machine / Gatekeeper smoke** (`PLAN.md` Phase 6 exit + §27): install the notarized zip on a fresh macOS user, open under Gatekeeper, run the functional smoke list (launch → open repo → stage → commit → history → branch → fetch/pull/push → failed command → relaunch → light/dark).
- [ ] **Publish a dedicated security contact** (`SECURITY.md` still says “until a dedicated security contact is published”). `PLAN.md` §27 Open source requires it.
- [ ] **Align product version with a GitHub tag** before calling the download line 1.0: bump `APP_VERSION` + packager version together, tag, confirm universal zip + `SHA256SUMS.txt` + stapled ticket.

### 2.2 Hosting (only GitHub Cloud PAT today)

`PLAN.md` Phase 9 last unchecked item:

- [ ] Enterprise / self-hosted GitHub (custom API endpoint).
- [ ] OAuth device flow (replace or complement the Keychain PAT dialog). Out of 1.0 if time-boxed; then say so in README known limitations **without** also claiming “notarization incomplete”.

GitLab, Bitbucket, Azure DevOps stay **post-v1**.

### 2.3 Git UX still partial

Operations exist; guided flows and a few surfaces do not.

- [ ] **Merge wizard** — choose ours/theirs/commit message, show in-progress banner, abort/continue from one place (today: merge action + recovery journal, not a wizard).
- [ ] **Rebase wizard** — same for rebase onto / continue / skip / abort. History “Edit Message” is still disabled (`docs/desktop-shell.md`).
- [ ] **Deterministic remote progress** — parse Git `--progress` byte/object lines everywhere; activity strip is still partly indeterminate (`CHANGELOG`, `UI-IMPROVE` §1.5).
- [ ] **History ref labels** — long names truncate on narrow panes (`UI-IMPROVE` remaining gaps).
- [ ] **Branches Review** — palette destination exists; full diverged-branch workflow is deferred (`UI-PLAN` explicit deferrals).
- [ ] **Git LFS operations** — status view exists; fetch/pull/push of LFS objects as first-class UI is not called out as done.
- [ ] **Commit signing UI** — signed-commit **status** is in Phase 8; a composer control to sign / explain missing key is not a 1.0 requirement unless the audit shows users cannot tell why a commit is unsigned.

### 2.4 Accessibility and keyboard

- [ ] VoiceOver / a11y: blocked on **GPUI 0.2.2** (no roles/labels). For v1, keep the limitation in `docs/troubleshooting.md` and do not claim “accessible” in README. Revisit only with a GPUI upgrade PR.
- [ ] Confirm in-app `Command-/` overlay matches `docs/keyboard-shortcuts.md` (no silent new bindings).

### 2.5 Branding / packaging polish

- [ ] Dock icon from `icon.png` → `assets/gitronimo.icns` at **all icns sizes** (`PLAN.md` §27). Visual check on Dock, Finder, About (`assets/gitronimo-icon.png`).
- [ ] `./bin/build` vs tag workflow: document that local apps are unsigned; only `v*` is Gatekeeper-clean.
- [ ] Entitlements / Hardened Runtime: confirm the release workflow’s flags match `PLAN.md` §27 (minimized entitlements).

---

## 3. Nice for v1, not blockers

Named in docs as deferred; ship if cheap, otherwise keep in known limitations.

- [ ] Workflow: Graphite CLI, git-flow CLI, stacked restack, auto-archive protection (`docs/desktop-shell.md`).
- [ ] Auto-stash / Snapshots / stash DnD (`docs/UI-IMPROVE.md`).
- [ ] Light + dark screenshot regression pass (blocked until screenshot inventory is restored — §1.1).
- [ ] `PLAN.md` §27 functional smoke as a **written** QA script (even if not automated).
- [ ] Refresh `docs/third-party-notices.md` against current `Cargo.lock` before the 1.0 tag.
- [ ] GPUI upgrade (own PR): future-incompat warnings for transitive `block` / `proc-macro-error2` (`docs/dependency-policy.md`). Not required for 1.0.

---

## 4. Explicitly out of v1

From `PLAN.md` §3.2 / §3.3 and `UI-PLAN` deferrals. Do not start these while §2.1 is open.

| Item | Why |
|------|-----|
| Localization | Post-MVP |
| In-app update system | Post-MVP |
| AI commit-message assistance | Post-MVP |
| GitLab / Bitbucket / Azure hosting | Phase 9 was GitHub-only |
| Pixel-identical clone of another Git client | Non-goal |
| Windows / Linux packaging | Non-goal |
| Built-in terminal or editor | Non-goal |
| Cloud settings sync | Non-goal |
| Custom Git transport / credential store / OpenSSH | Non-goal |

---

## 5. Suggested order toward 1.0

1. **Doc truth (§1)** — fix screens links, changelog notarization lie, PLAN gpui-component, UI-IMPROVE stale gaps, README zip name.
2. **Evidence (§2.1)** — tick `PLAN.md` §4 / §18 / Phase 6 release items that are already true; file bugs for any that fail.
3. **Ship or un-claim 0.9.2** — tag matches About, or README says “latest”.
4. **Git UX (§2.3)** — merge/rebase wizards and progress parsing if 1.0 still wants “no Terminal for recovery”.
5. **Hosting (§2.2)** — enterprise GitHub only if a real user needs it; otherwise document PAT + github.com only.
6. **Clean-machine Gatekeeper smoke + security contact + 1.0 tag.**

---

## 6. Doc map (what each file is for now)

| File | Keep as | Problem |
|------|---------|---------|
| `PLAN.md` | Contract + historical phases | Looks unfinished; §5.1 obsolete |
| `docs/UI-PLAN.md` | Done UI roadmap | Stale test count, dead screenshot paths |
| `docs/UI-IMPROVE.md` | GitRonimo view patterns | Remaining-gaps table stale |
| `CHANGELOG.md` | Release notes | Copy-pasted limitations across versions |
| `README.md` | User entry | May advertise an unreleased zip |
| `docs/work-log.md` | Per-task intent | Too large to scan |
| `docs/packaging.md` | How to build/sign | Accurate for 0.9.2 |
| `docs/desktop-shell.md` | Chrome behavior | Accurate; dated header |
| `docs/architecture.md` | Layers | Accurate |
| `SECURITY.md` | Vuln process | No contact yet |
| `TRADEMARKS.md` | Name/logo | Too thin vs GitHub mentions |
| `docs/screens/*` | Screenshot inventory | Deleted; links remain |
