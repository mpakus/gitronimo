# Third-party notices

Gitronimo is built with Rust crates recorded in `Cargo.lock`, including GPUI and its transitive dependencies. License and source policy checks run through `cargo deny check`.

Git operations use gitoxide `gix` (Apache-2.0 / MIT) as the default engine for discovery, status, history, stage/commit, and HTTPS fetch/clone. The installed Git executable remains required as fallback and for workflows `gix` does not orchestrate (checkout, merge, rebase, stash, push, hooks, signing, filters, LFS, SSH). Gitronimo does not bundle, modify, or replace Git's credential helpers, SSH handling, hooks, signing, filters, or LFS behavior.
