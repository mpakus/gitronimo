//! Commit history via `gix` rev-walk.
//!
//! gitui (MIT) `asyncgit/src/sync/logwalker.rs` walks with `gix` and commit-time
//! order; this mapper is original and matches `git_cli` pagination.

use app_core::GitBackendError;
use git_domain::{CommitIdentity, HistoryCommit, HistoryPage, HistoryReference, HistoryRequest};
use gix::bstr::ByteSlice;
use gix::revision::walk::Sorting;

use crate::gix_error;

/// Loads one history page from `gix` rev-walk.
///
/// # Errors
/// Returns when tips cannot be resolved or commit objects cannot be decoded.
pub(crate) fn history_page(
    repo: &gix::Repository,
    request: &HistoryRequest,
) -> Result<HistoryPage, GitBackendError> {
    let limit = request.limit.clamp(1, 500);
    let all_refs = matches!(request.reference, HistoryReference::All);
    let all_refs_skip = request
        .before
        .as_deref()
        .and_then(|cursor| cursor.strip_prefix("all:"))
        .and_then(|skip| skip.parse::<usize>().ok())
        .unwrap_or(0);
    let tips = walk_tips(repo, request, all_refs)?;
    let mut commits = Vec::new();
    let mut skipped = 0_usize;
    let mut drop_first = !all_refs && request.before.is_some();
    for info in repo
        .rev_walk(tips)
        .sorting(Sorting::ByCommitTime(
            gix::traverse::commit::simple::CommitTimeOrder::NewestFirst,
        ))
        .all()
        .map_err(gix_error)?
    {
        let info = info.map_err(gix_error)?;
        if all_refs {
            if skipped < all_refs_skip {
                skipped = skipped.saturating_add(1);
                continue;
            }
        } else if drop_first {
            drop_first = false;
            continue;
        }
        commits.push(map_commit(&info)?);
        if commits.len() > limit {
            break;
        }
    }
    let next_before = (commits.len() > limit).then(|| {
        if all_refs {
            format!("all:{}", all_refs_skip + limit)
        } else {
            commits[limit - 1].oid.clone()
        }
    });
    commits.truncate(limit);
    Ok(HistoryPage {
        commits,
        next_before,
    })
}

fn walk_tips(
    repo: &gix::Repository,
    request: &HistoryRequest,
    all_refs: bool,
) -> Result<Vec<gix::ObjectId>, GitBackendError> {
    if all_refs {
        return all_ref_tips(repo);
    }
    let spec = request.before.as_deref().map_or_else(
        || match &request.reference {
            HistoryReference::Named(name) => name.clone(),
            HistoryReference::Current | HistoryReference::All => "HEAD".to_owned(),
        },
        ToOwned::to_owned,
    );
    let id = repo
        .rev_parse_single(gix::bstr::BStr::new(spec.as_bytes()))
        .map_err(gix_error)?;
    Ok(vec![id.detach()])
}

fn all_ref_tips(repo: &gix::Repository) -> Result<Vec<gix::ObjectId>, GitBackendError> {
    let platform = repo.references().map_err(gix_error)?;
    let mut tips = Vec::new();
    for reference in platform.all().map_err(gix_error)? {
        let mut reference = reference.map_err(gix_error)?;
        let Ok(id) = reference.peel_to_id() else {
            continue;
        };
        let oid = id.detach();
        if !tips.contains(&oid) {
            tips.push(oid);
        }
    }
    Ok(tips)
}

fn map_commit(info: &gix::revision::walk::Info<'_>) -> Result<HistoryCommit, GitBackendError> {
    let commit = info.object().map_err(gix_error)?;
    let message = commit.message().map_err(gix_error)?;
    let author = commit.author().map_err(gix_error)?;
    let committer = commit.committer().map_err(gix_error)?;
    Ok(HistoryCommit {
        oid: commit.id().to_string(),
        parents: info.parent_ids().map(|id| id.to_string()).collect(),
        author: identity(author)?,
        committer: identity(committer)?,
        subject: message.title.trim_end().to_vec(),
        body: message
            .body
            .map_or_else(Vec::new, |body| body.trim_end().to_vec()),
    })
}

fn identity(signature: gix::actor::SignatureRef<'_>) -> Result<CommitIdentity, GitBackendError> {
    Ok(CommitIdentity {
        name: signature.name.to_vec(),
        email: signature.email.to_vec(),
        timestamp: signature.time().map_err(gix_error)?.seconds,
    })
}
