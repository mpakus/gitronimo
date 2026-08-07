# Implementation work log

## 2026-08-06 — Phase 0 / issue 1: initialize workspace and CI

**Intent:** create the reproducible Rust workspace and quality gates required before the GPUI window spike.

**Files:** root Cargo configuration, minimal boundary crates, policy documents, ADR template, and CI workflows.

**Acceptance checks:** workspace metadata resolves; exact GPUI dependencies resolve from `ui_kit`; formatting, Clippy, tests, and cargo-deny have an executable local/CI command.

**Deferred:** GPUI window, menus, components, resizable panels, packaging, and Git process behavior are separate checklist items.

**Environment blocker:** the local GPUI build currently stops in its Metal shader build step because Xcode's Metal Toolchain is absent. Install it with `xcodebuild -downloadComponent MetalToolchain` before running workspace-wide compile, Clippy, or tests. This is a machine prerequisite, not a dependency or source failure.

**Resolved environment prerequisite:** Metal Toolchain 17F109 was installed. The workspace now builds and its full Clippy/test gates pass. `cargo-deny` is installed by CI but is not available as a local Cargo subcommand yet, so its local result remains pending. The lockfile is generated and ready for the initial commit; its checklist item stays open until that commit is made.
