//! OpenAI-compatible commit-message suggestion over `curl`.
//!
//! Git stays out of this module except for the staged diff the caller supplies.

use std::{ffi::OsString, process::Command};

use app_core::{chat_completions_url, parse_chat_completion};

/// Posts `body` to an OpenAI-compatible chat completions URL.
///
/// # Errors
/// Returns a user-facing sentence. Does not include the API key or response body.
pub(crate) fn request_chat_completion(
    url: &str,
    api_key: Option<&str>,
    body: &str,
) -> Result<String, String> {
    let url = chat_completions_url(url)?;
    let mut args = vec![
        OsString::from("--silent"),
        OsString::from("--show-error"),
        OsString::from("--include"),
        OsString::from("--request"),
        OsString::from("POST"),
        OsString::from("--max-time"),
        OsString::from("60"),
        OsString::from("--header"),
        OsString::from("Content-Type: application/json"),
        OsString::from("--header"),
        OsString::from("User-Agent: GitRonimo"),
    ];
    if url.starts_with("https://") {
        args.push(OsString::from("--proto"));
        args.push(OsString::from("=https"));
        args.push(OsString::from("--tlsv1.2"));
    }
    if let Some(key) = api_key.filter(|key| !key.is_empty()) {
        args.push(OsString::from("--header"));
        args.push(OsString::from(format!("Authorization: Bearer {key}")));
    }
    args.push(OsString::from("--data"));
    args.push(OsString::from(body));
    args.push(OsString::from(url));
    let output = Command::new("curl")
        .args(args)
        .output()
        .map_err(|_| "Could not suggest a commit message.".to_owned())?;
    if !output.status.success() {
        return Err("Could not suggest a commit message.".into());
    }
    let (status, json) = split_response(&output.stdout)
        .ok_or_else(|| "Could not suggest a commit message.".to_owned())?;
    if status == 401 || status == 403 {
        return Err("Could not suggest a commit message: the API key was refused.".into());
    }
    if !(200..300).contains(&status) {
        return Err(format!(
            "Could not suggest a commit message: the endpoint returned HTTP {status}."
        ));
    }
    parse_chat_completion(json)
}

fn split_response(bytes: &[u8]) -> Option<(u16, &str)> {
    let separator = bytes.windows(4).rposition(|window| window == b"\r\n\r\n")?;
    let header_bytes = &bytes[..separator];
    let body = std::str::from_utf8(&bytes[separator + 4..]).ok()?;
    let headers = String::from_utf8_lossy(header_bytes);
    let status = headers
        .lines()
        .rev()
        .find(|line| line.starts_with("HTTP/"))?
        .split_whitespace()
        .nth(1)?
        .parse()
        .ok()?;
    Some((status, body))
}

#[cfg(test)]
mod tests {
    use super::split_response;
    use app_core::chat_completions_request_body;

    #[test]
    fn request_body_is_built_without_a_live_network_call() {
        let body = chat_completions_request_body("gpt-4o-mini", "staged diff");
        assert!(body.contains("staged diff"));
        assert!(!body.contains("Authorization"));
    }

    #[test]
    fn parses_curl_include_status() {
        let (status, body) = split_response(
            b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n{\"ok\":true}",
        )
        .expect("response");
        assert_eq!(status, 200);
        assert_eq!(body, "{\"ok\":true}");
    }

    #[test]
    fn parses_final_status_after_continue() {
        let (status, body) =
            split_response(b"HTTP/1.1 100 Continue\r\n\r\nHTTP/1.1 200 OK\r\n\r\n{\"ok\":true}")
                .expect("continued");
        assert_eq!(status, 200);
        assert_eq!(body, "{\"ok\":true}");
    }

    #[test]
    fn split_response_rejects_missing_header_separator() {
        assert!(split_response(b"HTTP/1.1 200 OK\n{\"ok\":true}").is_none());
        assert!(split_response(b"").is_none());
    }
}
