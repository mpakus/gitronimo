//! GitHub's provider adapter. It owns HTTP and JSON details, not UI state.

use std::{ffi::OsString, process::Command};

use app_core::{HostingError, HostingService};
use git_domain::{
    HostedRepository, MergeMethod, PullRequestComment, PullRequestDetail, PullRequestFile,
    PullRequestState, PullRequestSummary, ServiceAccount,
};
use serde_json::Value;

mod releases;

pub use releases::{
    GITRONIMO_GITHUB_REPO, LatestRelease, ProductVersion, download_url_is_allowed,
    is_safe_asset_filename, parse_latest_release, parse_product_version, sha256_for_filename,
    version_is_newer, zip_name_for_tag,
};

const ACCEPT_HEADER: &str = "Accept: application/vnd.github+json";
const USER_AGENT_HEADER: &str = "User-Agent: GitRonimo";

#[derive(Clone, Debug)]
pub struct GitHubService {
    api_base: String,
}

impl Default for GitHubService {
    fn default() -> Self {
        Self::new("https://api.github.com")
    }
}

impl GitHubService {
    #[must_use]
    pub fn new(api_base: impl Into<String>) -> Self {
        Self {
            api_base: api_base.into(),
        }
    }

    fn request_json(&self, token: &str, path: &str) -> Result<Value, HostingError> {
        self.request_json_with(Some(token), "GET", path, None)
    }

    /// Latest stable GitHub release. Unauthenticated; do not send a PAT.
    ///
    /// # Errors
    /// Network, HTTP, or JSON errors from GitHub, including a missing zip/sums pair.
    pub fn latest_release(&self, owner_repo: &str) -> Result<LatestRelease, HostingError> {
        if !releases::is_owner_repo(owner_repo) {
            return Err(HostingError::Parse);
        }
        let value = self.request_json_with(
            None,
            "GET",
            &format!("/repos/{owner_repo}/releases/latest"),
            None,
        )?;
        parse_latest_release(&value)
    }

    fn request_json_with(
        &self,
        token: Option<&str>,
        method: &str,
        path: &str,
        body: Option<Value>,
    ) -> Result<Value, HostingError> {
        let url = format!(
            "{}/{}",
            self.api_base.trim_end_matches('/'),
            path.trim_start_matches('/')
        );
        let mut args = vec![
            OsString::from("--silent"),
            OsString::from("--show-error"),
            OsString::from("--include"),
            OsString::from("--request"),
            OsString::from(method),
            OsString::from("--header"),
            OsString::from(ACCEPT_HEADER),
            OsString::from("--header"),
            OsString::from(USER_AGENT_HEADER),
        ];
        if let Some(token) = token {
            args.push(OsString::from("--header"));
            args.push(OsString::from(format!("Authorization: Bearer {token}")));
        }
        if let Some(body) = body {
            args.push(OsString::from("--header"));
            args.push(OsString::from("Content-Type: application/json"));
            args.push(OsString::from("--data"));
            args.push(OsString::from(body.to_string()));
        }
        args.push(OsString::from(url));
        let output = Command::new("curl")
            .args(args)
            .output()
            .map_err(|_| HostingError::Network)?;
        if !output.status.success() {
            return Err(HostingError::Network);
        }
        let (status, headers, body) =
            split_response(&output.stdout).ok_or(HostingError::Network)?;
        if status == 401 {
            return Err(HostingError::Authentication);
        }
        if status == 403
            && headers
                .to_ascii_lowercase()
                .contains("x-ratelimit-remaining: 0")
        {
            return Err(HostingError::RateLimited {
                retry_after_seconds: None,
            });
        }
        if !(200..300).contains(&status) {
            return Err(HostingError::Api(format!("GitHub returned HTTP {status}.")));
        }
        serde_json::from_slice(body).map_err(|_| HostingError::Parse)
    }
}

impl HostingService for GitHubService {
    fn authenticate(&self, token: &str) -> Result<ServiceAccount, HostingError> {
        let value = self.request_json(token, "/user")?;
        Ok(ServiceAccount {
            provider: "GitHub".into(),
            login: value
                .get("login")
                .and_then(Value::as_str)
                .ok_or(HostingError::Parse)?
                .to_owned(),
            display_name: value
                .get("name")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned),
        })
    }

    fn repositories(&self, token: &str) -> Result<Vec<HostedRepository>, HostingError> {
        let value = self.request_json(token, "/user/repos?per_page=100&sort=updated")?;
        let repositories = value.as_array().ok_or(HostingError::Parse)?;
        repositories.iter().map(parse_repository).collect()
    }

    fn pull_requests(
        &self,
        token: &str,
        repository: &HostedRepository,
    ) -> Result<Vec<PullRequestSummary>, HostingError> {
        let path = format!(
            "/repos/{}/pulls?state=open&per_page=100",
            repository.full_name
        );
        let value = self.request_json(token, &path)?;
        value
            .as_array()
            .ok_or(HostingError::Parse)?
            .iter()
            .map(parse_pull_request_summary)
            .collect()
    }

    fn pull_request_detail(
        &self,
        token: &str,
        repository: &HostedRepository,
        number: u64,
    ) -> Result<PullRequestDetail, HostingError> {
        let prefix = format!("/repos/{}/pulls/{number}", repository.full_name);
        let summary_value = self.request_json(token, &prefix)?;
        let summary = parse_pull_request_summary(&summary_value)?;
        let body = summary_value
            .get("body")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        let files = self
            .request_json(token, &format!("{prefix}/files?per_page=100"))?
            .as_array()
            .ok_or(HostingError::Parse)?
            .iter()
            .map(parse_pull_request_file)
            .collect::<Result<Vec<_>, _>>()?;
        let comments = self
            .request_json(
                token,
                &format!(
                    "/repos/{}/issues/{number}/comments?per_page=100",
                    repository.full_name
                ),
            )?
            .as_array()
            .ok_or(HostingError::Parse)?
            .iter()
            .map(parse_pull_request_comment)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(PullRequestDetail {
            summary,
            body,
            files,
            comments,
        })
    }

    fn create_pull_request(
        &self,
        token: &str,
        repository: &HostedRepository,
        title: &str,
        body: &str,
        head: &str,
        base: &str,
    ) -> Result<PullRequestSummary, HostingError> {
        let value = self.request_json_with(
            Some(token),
            "POST",
            &format!("/repos/{}/pulls", repository.full_name),
            Some(serde_json::json!({
                "title": title,
                "body": body,
                "head": head,
                "base": base
            })),
        )?;
        parse_pull_request_summary(&value)
    }

    fn comment_pull_request(
        &self,
        token: &str,
        repository: &HostedRepository,
        number: u64,
        body: &str,
    ) -> Result<PullRequestComment, HostingError> {
        let value = self.request_json_with(
            Some(token),
            "POST",
            &format!("/repos/{}/issues/{number}/comments", repository.full_name),
            Some(serde_json::json!({ "body": body })),
        )?;
        parse_pull_request_comment(&value)
    }

    fn merge_pull_request(
        &self,
        token: &str,
        repository: &HostedRepository,
        number: u64,
        method: MergeMethod,
    ) -> Result<(), HostingError> {
        self.request_json_with(
            Some(token),
            "PUT",
            &format!("/repos/{}/pulls/{number}/merge", repository.full_name),
            Some(serde_json::json!({ "merge_method": method.api_name() })),
        )?;
        Ok(())
    }
}

fn parse_repository(value: &Value) -> Result<HostedRepository, HostingError> {
    let full_name = value
        .get("full_name")
        .and_then(Value::as_str)
        .ok_or(HostingError::Parse)?
        .to_owned();
    let owner = full_name
        .split_once('/')
        .map(|(owner, _)| owner.to_owned())
        .ok_or(HostingError::Parse)?;
    Ok(HostedRepository {
        id: value
            .get("id")
            .and_then(Value::as_u64)
            .ok_or(HostingError::Parse)?,
        owner,
        name: value
            .get("name")
            .and_then(Value::as_str)
            .ok_or(HostingError::Parse)?
            .to_owned(),
        full_name,
        clone_url: value
            .get("clone_url")
            .and_then(Value::as_str)
            .ok_or(HostingError::Parse)?
            .to_owned(),
        ssh_url: value
            .get("ssh_url")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        private: value
            .get("private")
            .and_then(Value::as_bool)
            .ok_or(HostingError::Parse)?,
    })
}

fn parse_pull_request_summary(value: &Value) -> Result<PullRequestSummary, HostingError> {
    let state = value
        .get("state")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let merged = value.get("merged_at").is_some_and(|value| !value.is_null());
    Ok(PullRequestSummary {
        number: value
            .get("number")
            .and_then(Value::as_u64)
            .ok_or(HostingError::Parse)?,
        title: value
            .get("title")
            .and_then(Value::as_str)
            .ok_or(HostingError::Parse)?
            .to_owned(),
        author: value
            .get("user")
            .and_then(|user| user.get("login"))
            .and_then(Value::as_str)
            .ok_or(HostingError::Parse)?
            .to_owned(),
        updated_at: value
            .get("updated_at")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        state: if merged {
            PullRequestState::Merged
        } else {
            match state {
                "open" => PullRequestState::Open,
                "closed" => PullRequestState::Closed,
                other => PullRequestState::Other(other.to_owned()),
            }
        },
        head_ref: value
            .get("head")
            .and_then(|head| head.get("ref"))
            .and_then(Value::as_str)
            .ok_or(HostingError::Parse)?
            .to_owned(),
        base_ref: value
            .get("base")
            .and_then(|base| base.get("ref"))
            .and_then(Value::as_str)
            .ok_or(HostingError::Parse)?
            .to_owned(),
    })
}

fn parse_pull_request_file(value: &Value) -> Result<PullRequestFile, HostingError> {
    Ok(PullRequestFile {
        path: value
            .get("filename")
            .and_then(Value::as_str)
            .ok_or(HostingError::Parse)?
            .to_owned(),
        additions: value
            .get("additions")
            .and_then(Value::as_u64)
            .ok_or(HostingError::Parse)?,
        deletions: value
            .get("deletions")
            .and_then(Value::as_u64)
            .ok_or(HostingError::Parse)?,
    })
}

fn parse_pull_request_comment(value: &Value) -> Result<PullRequestComment, HostingError> {
    Ok(PullRequestComment {
        author: value
            .get("user")
            .and_then(|user| user.get("login"))
            .and_then(Value::as_str)
            .ok_or(HostingError::Parse)?
            .to_owned(),
        body: value
            .get("body")
            .and_then(Value::as_str)
            .ok_or(HostingError::Parse)?
            .to_owned(),
        created_at: value
            .get("created_at")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
    })
}

fn split_response(bytes: &[u8]) -> Option<(u16, String, &[u8])> {
    let separator = bytes.windows(4).rposition(|window| window == b"\r\n\r\n")?;
    let header_bytes = &bytes[..separator];
    let body = &bytes[separator + 4..];
    let headers = String::from_utf8_lossy(header_bytes).into_owned();
    let status = headers
        .lines()
        .rev()
        .find(|line| line.starts_with("HTTP/"))?
        .split_whitespace()
        .nth(1)?
        .parse()
        .ok()?;
    Some((status, headers, body))
}

#[cfg(test)]
mod tests {
    use super::{
        parse_pull_request_comment, parse_pull_request_file, parse_pull_request_summary,
        parse_repository, split_response,
    };

    #[test]
    fn parses_github_response_status_and_body() {
        let (status, headers, body) =
            split_response(b"HTTP/1.1 200 OK\r\nX-Test: yes\r\n\r\n{\"ok\":true}")
                .expect("response should parse");
        assert_eq!(status, 200);
        assert!(headers.contains("X-Test: yes"));
        assert_eq!(body, b"{\"ok\":true}");
    }

    #[test]
    fn parses_provider_repository_without_secrets() {
        let value = serde_json::json!({
            "id": 42,
            "name": "demo",
            "full_name": "octo/demo",
            "clone_url": "https://github.com/octo/demo.git",
            "ssh_url": "git@github.com:octo/demo.git",
            "private": true
        });
        let repository = parse_repository(&value).expect("repository should parse");
        assert_eq!(repository.full_name, "octo/demo");
        assert_eq!(
            repository.ssh_url.as_deref(),
            Some("git@github.com:octo/demo.git")
        );
        assert!(repository.private);
    }

    #[test]
    fn parses_pull_request_summary_files_and_comments() {
        let value = serde_json::json!({
            "number": 7,
            "title": "Improve status",
            "state": "open",
            "updated_at": "2026-08-09T10:00:00Z",
            "user": {"login": "octocat"},
            "head": {"ref": "feature/status"},
            "base": {"ref": "main"},
            "body": "Description"
        });
        let summary = parse_pull_request_summary(&value).expect("pull request should parse");
        assert_eq!(summary.number, 7);
        assert_eq!(summary.head_ref, "feature/status");
        let file = parse_pull_request_file(&serde_json::json!({
            "filename": "src/lib.rs",
            "additions": 3,
            "deletions": 1
        }))
        .expect("file should parse");
        assert_eq!(file.additions, 3);
        let comment = parse_pull_request_comment(&serde_json::json!({
            "user": {"login": "reviewer"},
            "body": "Looks good",
            "created_at": "2026-08-09T10:01:00Z"
        }))
        .expect("comment should parse");
        assert_eq!(comment.author, "reviewer");
    }
}
