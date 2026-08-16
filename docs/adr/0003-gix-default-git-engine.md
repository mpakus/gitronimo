# ADR 0003: gix as the default Git engine, system Git as fallback

- Status: Accepted
- Date: 2026-08-16

## Context

GitRonimo 1.0 talks to Git only through `git_cli` (typed `std::process::Command` arguments to the installed executable). [`PLAN.md`](../../PLAN.md) allowed `gix` for read optimization after profiling and forbade `gix` mutations in the MVP. 2.0 ([`PLAN-v2.md`](../PLAN-v2.md)) makes [gitoxide](https://github.com/GitoxideLabs/gitoxide) `gix` the main internal engine so status, history, and refs do not spawn Git for every query, while operations `gix` does not orchestrate yet (checkout, merge, rebase, stash, push, hooks, LFS, signing) stay on system Git.

`git_domain` must remain types-only. Application code must not import the `gix` crate or build Git command lines in GPUI.

## Decision

- Default engine is the **`gix` library crate** (not the `gix` CLI, not `gitoxide-core`), pinned exactly, consumed only from `git_gix`.
- `git_cli` remains the fallback and the Settings override (“Use system Git”).
- Ports live in `app_core` (`RepositoryDiscoverer`, `GitRefQuery`, `GitHistoryQuery`, `GitObjectQuery`, `GitIndexMutate`, `GitNetwork`). Desktop composes `gix` then `git_cli`; it does not call `gix` or `Command` for migrated operations.
- Capability follows [crate-status.md (`gix`)](https://github.com/GitoxideLabs/gitoxide/blob/main/crate-status.md#gix). Missing workflows stay on `git_cli` until a dedicated work-log unit migrates them with dual-backend tests.
- Prefer `gix::discover_with_environment_overrides` so `GIT_DIR` / `GIT_WORK_TREE` behave like Git (same approach as gitui, MIT).

## Alternatives

- Keep spawning Git for every read: rejected for 2.0; it is the 1.0 baseline and the fallback.
- Replace system Git entirely: rejected; `gix` still lacks checkout/merge/rebase/stash/push orchestration.
- Put `gix` types in `git_domain`: rejected; domain stays UI- and library-independent.
- Use `git2` / libgit2: rejected; 2.0 standardizes on gitoxide.

## Consequences

Open, HEAD, refs, working-copy status, history, tree/blob reads, unified diffs, stage/unstage/commit, and HTTPS fetch/clone use `gix` unless the user forces system Git or `gix` returns an error, in which case `git_cli` runs. Dual-backend tests compare `git_domain` models (history OIDs and subjects, tree entries, diff hunk line kinds, stage/commit/amend). `cargo deny` must stay green on the `gix` graph.

Known deviations vs porcelain: status entries are sorted by path; rename score is 100; typechange is worktree `T`; submodule nested untracked is not listed; diff hunk headers may omit or include `,1` unlike Git; tree directories use mode `040000`. `gix` commit does not run hooks or GPG-sign — if executable commit hooks or `commit.gpgsign` are set, the adapter errors so desktop falls back to system Git. SSH and `file://` fetch/clone are refused by `git_gix` so fallback runs. HTTPS fetch cancel uses `AtomicBool`; SSH fetch still uses `GitChild`. Status queries do not write index racy-stat updates. Hunk staging, discard, checkout, merge, rebase, stash mutations, and push stay on system Git.

## Rollback path

Set the Settings override on by default (or remove `git_gix` from the desktop crate), keep `git_cli` call sites, and replace this ADR.
