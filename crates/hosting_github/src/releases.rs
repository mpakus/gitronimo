//! Public GitHub Releases helpers for the in-app updater.
//!
//! JSON field names follow GitHub's `/releases/latest` payload (same approach as
//! rgitui's MIT `update_checker.rs`). Version compare is a three-part tuple; no
//! `semver` crate. `GitComet` announce-only is AGPL — not used here.

use std::fmt;

use app_core::HostingError;
use serde_json::Value;

/// Repository that publishes notarized `GitRonimo-v*.zip` assets.
pub const GITRONIMO_GITHUB_REPO: &str = "mpakus/gitronimo";

const SHA256SUMS_NAME: &str = "SHA256SUMS.txt";

/// Parsed product version (`1.0.0` or `v1.0.1-rc.1`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProductVersion {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
    /// True when the string has a suffix after the patch number (`-rc.1`, `+meta`).
    pub prerelease: bool,
}

impl ProductVersion {
    #[must_use]
    pub const fn tuple(self) -> (u64, u64, u64) {
        (self.major, self.minor, self.patch)
    }
}

impl fmt::Display for ProductVersion {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// Assets required to install a `GitRonimo` GitHub release.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LatestRelease {
    pub tag: String,
    pub version: ProductVersion,
    pub zip_name: String,
    pub zip_url: String,
    pub sums_url: String,
}

/// Parses `1.2.3`, `v1.2.3`, or a prerelease such as `v1.2.3-rc.1`.
#[must_use]
pub fn parse_product_version(raw: &str) -> Option<ProductVersion> {
    let trimmed = raw.trim();
    let without_v = trimmed
        .strip_prefix('v')
        .or_else(|| trimmed.strip_prefix('V'))
        .unwrap_or(trimmed);
    let mut parts = without_v.splitn(3, '.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch_and_pre = parts.next()?;
    let patch_len = patch_and_pre
        .find(|ch: char| !ch.is_ascii_digit())
        .unwrap_or(patch_and_pre.len());
    if patch_len == 0 {
        return None;
    }
    let patch = patch_and_pre[..patch_len].parse().ok()?;
    let prerelease = patch_len < patch_and_pre.len();
    Some(ProductVersion {
        major,
        minor,
        patch,
        prerelease,
    })
}

/// Whether `candidate` is a newer installable release than `current`.
///
/// Prereleases are ignored unless the running app is also a prerelease.
#[must_use]
pub fn version_is_newer(current: ProductVersion, candidate: ProductVersion) -> bool {
    if candidate.prerelease && !current.prerelease {
        return false;
    }
    match candidate.tuple().cmp(&current.tuple()) {
        std::cmp::Ordering::Greater => true,
        std::cmp::Ordering::Equal => current.prerelease && !candidate.prerelease,
        std::cmp::Ordering::Less => false,
    }
}

/// Zip name published by `.github/workflows/release.yml` (`GitRonimo-${tag}.zip`).
#[must_use]
pub fn zip_name_for_tag(tag: &str) -> String {
    format!("GitRonimo-{tag}.zip")
}

/// HTTPS GitHub release-asset hosts only. Rejects userinfo and whitespace.
#[must_use]
pub fn download_url_is_allowed(url: &str) -> bool {
    let url = url.trim();
    if url.is_empty() || url.contains(['\n', '\r', ' ', '\t', '@']) {
        return false;
    }
    [
        "https://github.com/",
        "https://objects.githubusercontent.com/",
        "https://release-assets.githubusercontent.com/",
    ]
    .iter()
    .any(|prefix| url.starts_with(prefix))
}

/// Parses GitHub `/releases/latest` JSON into zip + checksum asset URLs.
///
/// # Errors
/// Returns [`HostingError::Parse`] when required fields or assets are missing.
pub fn parse_latest_release(value: &Value) -> Result<LatestRelease, HostingError> {
    let tag = value
        .get("tag_name")
        .and_then(Value::as_str)
        .ok_or(HostingError::Parse)?
        .to_owned();
    if tag.is_empty() || tag.contains(['/', '\\', '\0']) {
        return Err(HostingError::Parse);
    }
    let version = parse_product_version(&tag).ok_or(HostingError::Parse)?;
    if value
        .get("prerelease")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return Err(HostingError::Api(
            "Latest GitHub release is marked prerelease.".into(),
        ));
    }
    let zip_name = zip_name_for_tag(&tag);
    let assets = value
        .get("assets")
        .and_then(Value::as_array)
        .ok_or(HostingError::Parse)?;
    let zip_url = asset_download_url(assets, &zip_name)?;
    let sums_url = asset_download_url(assets, SHA256SUMS_NAME)?;
    Ok(LatestRelease {
        tag,
        version,
        zip_name,
        zip_url,
        sums_url,
    })
}

fn asset_download_url(assets: &[Value], name: &str) -> Result<String, HostingError> {
    for asset in assets {
        let asset_name = asset
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if asset_name != name {
            continue;
        }
        let url = asset
            .get("browser_download_url")
            .and_then(Value::as_str)
            .ok_or(HostingError::Parse)?;
        if !download_url_is_allowed(url) {
            return Err(HostingError::Api(
                "Release asset URL is not a GitHub HTTPS download.".into(),
            ));
        }
        return Ok(url.to_owned());
    }
    Err(HostingError::Api(format!(
        "Latest release is missing {name}."
    )))
}

/// Looks up a 64-hex SHA-256 for `filename` in `shasum -a 256` text.
///
/// # Errors
/// Returns parse errors for path traversal in the filename or a missing entry.
pub fn sha256_for_filename(text: &str, filename: &str) -> Result<String, HostingError> {
    if !is_safe_asset_filename(filename) {
        return Err(HostingError::Parse);
    }
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((hash, name)) = parse_sum_line(line) else {
            continue;
        };
        if name == filename {
            return Ok(hash);
        }
    }
    Err(HostingError::Api(format!(
        "SHA256SUMS.txt has no entry for {filename}."
    )))
}

#[must_use]
pub fn is_safe_asset_filename(name: &str) -> bool {
    !name.is_empty()
        && !name.contains(['/', '\\', '\0'])
        && !name.contains("..")
        && name
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-' | '_'))
}

fn parse_sum_line(line: &str) -> Option<(String, String)> {
    let hash = line.get(..64)?;
    if !hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    let rest = line.get(64..)?.trim_start();
    let name = rest.strip_prefix('*').unwrap_or(rest).trim();
    if !is_safe_asset_filename(name) {
        return None;
    }
    Some((hash.to_ascii_lowercase(), name.to_owned()))
}

#[must_use]
pub fn is_owner_repo(value: &str) -> bool {
    let mut parts = value.split('/');
    let Some(owner) = parts.next() else {
        return false;
    };
    let Some(repo) = parts.next() else {
        return false;
    };
    parts.next().is_none() && is_github_name(owner) && is_github_name(repo)
}

fn is_github_name(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
}

#[cfg(test)]
mod tests {
    use super::{
        download_url_is_allowed, parse_latest_release, parse_product_version, sha256_for_filename,
        version_is_newer, zip_name_for_tag,
    };
    use app_core::HostingError;

    #[test]
    fn parses_versions_and_compares_newer() {
        let current = parse_product_version("1.0.0").expect("current");
        let patch = parse_product_version("v1.0.1").expect("patch");
        let minor = parse_product_version("1.1.0").expect("minor");
        let major = parse_product_version("2.0.0").expect("major");
        let rc = parse_product_version("v1.0.1-rc.1").expect("rc");
        assert!(version_is_newer(current, patch));
        assert!(version_is_newer(current, minor));
        assert!(version_is_newer(current, major));
        assert!(!version_is_newer(patch, current));
        assert!(!version_is_newer(current, current));
        assert!(!version_is_newer(current, rc));
        let current_rc = parse_product_version("1.0.0-rc.1").expect("current rc");
        assert!(version_is_newer(current_rc, current));
        assert!(parse_product_version("1.0").is_none());
    }

    #[test]
    fn zip_name_matches_release_workflow() {
        assert_eq!(zip_name_for_tag("v1.0.1"), "GitRonimo-v1.0.1.zip");
    }

    #[test]
    fn allows_github_https_asset_hosts_only() {
        assert!(download_url_is_allowed(
            "https://github.com/mpakus/gitronimo/releases/download/v1.0.1/GitRonimo-v1.0.1.zip"
        ));
        assert!(download_url_is_allowed(
            "https://objects.githubusercontent.com/github-production-release-asset-2e65be/file"
        ));
        assert!(!download_url_is_allowed(
            "http://github.com/mpakus/gitronimo/releases/download/v1.0.1/GitRonimo-v1.0.1.zip"
        ));
        assert!(!download_url_is_allowed(
            "https://evil.example/GitRonimo.zip"
        ));
        assert!(!download_url_is_allowed(
            "https://github.com.evil.example/GitRonimo.zip"
        ));
        assert!(!download_url_is_allowed(
            "https://user@github.com/mpakus/gitronimo/releases/download/v1.0.1/x.zip"
        ));
    }

    #[test]
    fn parses_release_json_assets() {
        let value = serde_json::json!({
            "tag_name": "v1.0.1",
            "prerelease": false,
            "assets": [
                {
                    "name": "GitRonimo-v1.0.1.zip",
                    "browser_download_url": "https://github.com/mpakus/gitronimo/releases/download/v1.0.1/GitRonimo-v1.0.1.zip"
                },
                {
                    "name": "SHA256SUMS.txt",
                    "browser_download_url": "https://github.com/mpakus/gitronimo/releases/download/v1.0.1/SHA256SUMS.txt"
                }
            ]
        });
        let release = parse_latest_release(&value).expect("release");
        assert_eq!(release.tag, "v1.0.1");
        assert_eq!(release.zip_name, "GitRonimo-v1.0.1.zip");
        assert_eq!(release.version.to_string(), "1.0.1");
    }

    #[test]
    fn rejects_prerelease_and_missing_zip() {
        let prerelease = serde_json::json!({
            "tag_name": "v1.0.1",
            "prerelease": true,
            "assets": []
        });
        assert!(matches!(
            parse_latest_release(&prerelease),
            Err(HostingError::Api(_))
        ));
        let missing = serde_json::json!({
            "tag_name": "v1.0.1",
            "prerelease": false,
            "assets": [{
                "name": "SHA256SUMS.txt",
                "browser_download_url": "https://github.com/mpakus/gitronimo/releases/download/v1.0.1/SHA256SUMS.txt"
            }]
        });
        assert!(matches!(
            parse_latest_release(&missing),
            Err(HostingError::Api(_))
        ));
    }

    #[test]
    fn parses_sha256sums_and_rejects_path_traversal() {
        let text = "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789  GitRonimo-v1.0.1.zip\n";
        assert_eq!(
            sha256_for_filename(text, "GitRonimo-v1.0.1.zip").expect("hash"),
            "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789"
        );
        assert!(sha256_for_filename(text, "../GitRonimo-v1.0.1.zip").is_err());
        let slip =
            "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789  ../evil.zip\n";
        assert!(sha256_for_filename(slip, "evil.zip").is_err());
    }
}
