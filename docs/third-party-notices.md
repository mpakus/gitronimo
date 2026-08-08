# Third-party notices

Gitronimo is built with Rust crates recorded in `Cargo.lock`, including GPUI and its transitive dependencies. License and source policy checks run through `cargo deny check`.

Git operations require an installed Git executable. Gitronimo does not bundle, modify, or replace Git's credential helpers, SSH handling, hooks, signing, filters, or LFS behavior.
