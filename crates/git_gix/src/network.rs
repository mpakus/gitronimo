//! HTTPS fetch and clone via `gix`. SSH and `file://` stay on system Git.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use app_core::GitBackendError;
use gix::bstr::BStr;
use gix::remote::Direction;
use gix::url::Scheme;

use crate::gix_error;

/// Whether `gix` should handle this clone/fetch URL (HTTP or HTTPS only).
#[must_use]
pub fn uses_http_url(url: &str) -> bool {
    let trimmed = url.trim();
    let lower = trimmed.to_ascii_lowercase();
    lower.starts_with("https://") || lower.starts_with("http://")
}

/// Fetches `remote` when its fetch URL is HTTP(S).
///
/// # Errors
/// Returns when the remote is not HTTP(S), fetch fails, or `interrupt` is set.
pub(crate) fn fetch_remote(
    repo: &gix::Repository,
    remote: &str,
    interrupt: &AtomicBool,
) -> Result<(), GitBackendError> {
    check_interrupt(interrupt)?;
    let remote = repo
        .find_remote(BStr::new(remote.as_bytes()))
        .map_err(gix_error)?;
    let url = remote
        .url(Direction::Fetch)
        .ok_or_else(|| GitBackendError::from_message("remote has no fetch URL"))?;
    if !matches!(url.scheme, Scheme::Https | Scheme::Http) {
        return Err(GitBackendError::from_message(
            "SSH and file fetch require system Git",
        ));
    }
    let connection = remote.connect(Direction::Fetch).map_err(gix_error)?;
    connection
        .prepare_fetch(
            gix::progress::Discard,
            gix::remote::ref_map::Options::default(),
        )
        .map_err(gix_error)?
        .receive(gix::progress::Discard, interrupt)
        .map_err(gix_error)?;
    check_interrupt(interrupt)
}

/// Clones an HTTP(S) URL into `destination`.
///
/// # Errors
/// Returns when the URL is not HTTP(S), clone fails, or `interrupt` is set.
pub(crate) fn clone_repository(
    source: &str,
    destination: &Path,
    interrupt: &AtomicBool,
) -> Result<(), GitBackendError> {
    if !uses_http_url(source) {
        return Err(GitBackendError::from_message(
            "SSH and file clone require system Git",
        ));
    }
    check_interrupt(interrupt)?;
    let mut prepare = gix::prepare_clone(source, destination).map_err(gix_error)?;
    let (mut checkout, _outcome) = prepare
        .fetch_then_checkout(gix::progress::Discard, interrupt)
        .map_err(gix_error)?;
    checkout
        .main_worktree(gix::progress::Discard, interrupt)
        .map_err(gix_error)?;
    check_interrupt(interrupt)
}

fn check_interrupt(interrupt: &AtomicBool) -> Result<(), GitBackendError> {
    if interrupt.load(Ordering::Relaxed) {
        Err(GitBackendError::from_message("cancelled"))
    } else {
        Ok(())
    }
}
