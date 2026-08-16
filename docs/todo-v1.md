# GitRonimo 1.0.0

**Status:** in-tree product is **1.0.0**  
**As of:** 2026-08-14

The daily Git client is already in the app. This file is what is left **after** the 1.0.0 version bump — mostly a tag and a smoke pass — plus work that is explicitly **not** 1.0.0.

---

## 1.0.0 cut (done in tree)

- Working Copy, History, branches, stashes, remotes, GitHub PRs (PAT), Settings, Workflow
- Fetch / pull / push with cancel; Pull/Push dialogs; force-with-lease confirm
- Merge / rebase / cherry-pick / revert / reset as operations; Working Copy continue/abort banner; Interactive rebase view
- Command palette, message history, About GitRonimo **1.0.0**
- Universal signed/notarized **tag** workflow (`.github/workflows/release.yml`)
- Network bar follows Git `%` when Git prints it

---

## You still do by hand today

- [x] `cargo fmt --all -- --check` && clippy `-D warnings` && `cargo test --workspace --all-features` && `cargo deny check` (2026-08-14, Rust 1.97.1)
- [x] `./bin/build` — unsigned `target/release-arm/GitRonimo.app` (`CFBundleShortVersionString` 1.0.0, `arm64`)
- [ ] Open that app once (launch, open repo, stage, commit, history, light/dark)
- [ ] Commit the 1.0.0 bump, tag **`v1.0.0`**, push the tag (CI notarizes the universal zip)
- [ ] Confirm [GitHub Releases](https://github.com/mpakus/gitronimo/releases/latest) shows `GitRonimo-v1.0.0.zip` + `SHA256SUMS.txt`
- [ ] Optional: install that zip on a clean macOS user and pass Gatekeeper

`PLAN.md` §27 functional smoke and clean-machine boxes stay open until those steps happen.

---

## Not 1.0.0 (do not start while tagging)

Tracked as **[`PLAN-v2.md`](PLAN-v2.md)** — `gix` as primary Git (system Git fallback), in-app updater, LFS fetch/pull UI and stash extras, optional AI commit messages.

---

## Security leftovers (`PLAN.md` §18)

Closed for 1.0.0: URL redaction in Git stderr/activity, commit-message `0o600` + unlink, `--no-ext-diff --no-textconv` on internal diffs (including numstat), diagnostics export is Git version only (no env dump), repository discovery canonicalizes via `git rev-parse`.
