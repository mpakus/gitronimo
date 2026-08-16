//! OpenAI-compatible commit-message suggestion helpers.
//!
//! Network I/O stays in the desktop crate. This module builds the prompt and
//! parses JSON. The prompt is the staged diff only — no repository files, PAT,
//! or extra context.

use serde_json::Value;

/// Default OpenAI-compatible base (no trailing slash).
pub const DEFAULT_AI_COMMIT_ENDPOINT: &str = "https://api.openai.com/v1";
/// Default chat model when Settings leaves the field empty.
pub const DEFAULT_AI_COMMIT_MODEL: &str = "gpt-4o-mini";
/// Cap for staged diff text sent to the model.
pub const MAX_AI_COMMIT_DIFF_BYTES: usize = 48_000;

/// HTTPS remotes, or HTTP only on loopback.
#[must_use]
fn ai_commit_endpoint_is_allowed(url: &str) -> bool {
    let url = url.trim();
    if url.is_empty() || url.contains(['\n', '\r', ' ', '\t', '@']) {
        return false;
    }
    if url.starts_with("https://") {
        return true;
    }
    loopback_http_url(url)
}

fn loopback_http_url(url: &str) -> bool {
    ["127.0.0.1", "localhost", "[::1]"].into_iter().any(|host| {
        let prefix = format!("http://{host}");
        url == prefix
            || url.starts_with(&format!("{prefix}:"))
            || url.starts_with(&format!("{prefix}/"))
    })
}

/// HTTPS (including the empty-settings default) needs a Keychain API key.
/// Loopback HTTP does not.
#[must_use]
pub fn ai_commit_requires_api_key(endpoint: &str) -> bool {
    let resolved = if endpoint.trim().is_empty() {
        DEFAULT_AI_COMMIT_ENDPOINT
    } else {
        endpoint.trim()
    };
    resolved.starts_with("https://")
}

/// Resolves `{base}/chat/completions` unless the base already ends with that path.
///
/// # Errors
/// Returns an error when the URL is empty or not on the allowlist.
pub fn chat_completions_url(base: &str) -> Result<String, String> {
    let trimmed = base.trim().trim_end_matches('/');
    let resolved = if trimmed.is_empty() {
        DEFAULT_AI_COMMIT_ENDPOINT.to_owned()
    } else {
        trimmed.to_owned()
    };
    if !ai_commit_endpoint_is_allowed(&resolved) {
        return Err("Could not suggest a commit message: the endpoint is not allowed.".into());
    }
    if resolved.ends_with("/chat/completions") {
        Ok(resolved)
    } else {
        Ok(format!("{resolved}/chat/completions"))
    }
}

/// Builds the user prompt. `diff` must already be redacted by the caller.
#[must_use]
pub fn build_commit_suggestion_prompt(diff: &str) -> String {
    format!(
        "Write a Git commit message for this staged diff only.\n\
         First line: a subject of at most 72 characters.\n\
         Then a blank line and an optional body.\n\
         Output only the commit message. No markdown fences, no commentary.\n\n\
         Staged diff:\n{diff}"
    )
}

/// JSON body for `POST /chat/completions`. Does not include an API key.
#[must_use]
pub fn chat_completions_request_body(model: &str, prompt: &str) -> String {
    let model = if model.trim().is_empty() {
        DEFAULT_AI_COMMIT_MODEL
    } else {
        model.trim()
    };
    serde_json::json!({
        "model": model,
        "messages": [{"role": "user", "content": prompt}],
        "temperature": 0.2,
        "max_tokens": 400
    })
    .to_string()
}

/// Reads `choices[0].message.content` from an OpenAI-compatible response.
///
/// # Errors
/// Returns an error when the payload is not a chat completion with text.
pub fn parse_chat_completion(json: &str) -> Result<String, String> {
    let value: Value = serde_json::from_str(json)
        .map_err(|_| "Could not parse the suggestion response.".to_owned())?;
    let content = value
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|choices| choices.first())
        .and_then(|choice| choice.get("message"))
        .and_then(|message| message.get("content"))
        .and_then(Value::as_str)
        .ok_or_else(|| {
            "Could not read a commit message from the suggestion response.".to_owned()
        })?;
    let text = strip_markdown_fences(content).trim().to_owned();
    if text.is_empty() {
        Err("The suggestion was empty.".into())
    } else {
        Ok(text)
    }
}

/// Splits a model reply into commit subject and body.
#[must_use]
pub fn split_commit_message(text: &str) -> (String, String) {
    let text = strip_markdown_fences(text).trim().to_owned();
    let (first, rest) = text
        .split_once('\n')
        .map_or((text.as_str(), ""), |(first, rest)| (first, rest));
    let subject = first.trim().trim_matches('`').to_owned();
    let body = rest.trim().to_owned();
    (subject, body)
}

fn strip_markdown_fences(text: &str) -> String {
    let trimmed = text.trim();
    let Some(without_open) = trimmed.strip_prefix("```") else {
        return trimmed.to_owned();
    };
    let without_lang = without_open
        .split_once('\n')
        .map_or(without_open, |(_, rest)| rest);
    without_lang
        .trim_end()
        .strip_suffix("```")
        .unwrap_or(without_lang)
        .trim()
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::{
        DEFAULT_AI_COMMIT_ENDPOINT, DEFAULT_AI_COMMIT_MODEL, ai_commit_endpoint_is_allowed,
        ai_commit_requires_api_key, build_commit_suggestion_prompt, chat_completions_request_body,
        chat_completions_url, parse_chat_completion, split_commit_message,
    };

    #[test]
    fn allows_https_and_loopback_http_only() {
        assert!(ai_commit_endpoint_is_allowed("https://api.openai.com/v1"));
        assert!(ai_commit_endpoint_is_allowed("http://127.0.0.1:11434/v1"));
        assert!(ai_commit_endpoint_is_allowed("http://localhost:11434/v1"));
        assert!(ai_commit_endpoint_is_allowed("http://[::1]:11434/v1"));
        assert!(ai_commit_endpoint_is_allowed("http://127.0.0.1/v1"));
        assert!(!ai_commit_endpoint_is_allowed(
            "http://127.0.0.1.evil.com/v1"
        ));
        assert!(!ai_commit_endpoint_is_allowed(
            "http://localhost.evil.com/v1"
        ));
        assert!(!ai_commit_endpoint_is_allowed("http://evil.example/v1"));
        assert!(!ai_commit_endpoint_is_allowed("http://example.local/v1"));
        assert!(!ai_commit_endpoint_is_allowed("file:///etc/passwd"));
        assert!(!ai_commit_endpoint_is_allowed(
            "https://api.openai.com/v1\nhttps://evil"
        ));
        assert!(!ai_commit_endpoint_is_allowed(
            "https://user:pass@api.openai.com/v1"
        ));
    }

    #[test]
    fn completions_url_appends_path() {
        assert_eq!(
            chat_completions_url("https://api.openai.com/v1").expect("url"),
            "https://api.openai.com/v1/chat/completions"
        );
        assert_eq!(
            chat_completions_url("https://api.openai.com/v1/chat/completions").expect("url"),
            "https://api.openai.com/v1/chat/completions"
        );
        assert_eq!(
            chat_completions_url("").expect("empty uses default"),
            format!("{DEFAULT_AI_COMMIT_ENDPOINT}/chat/completions")
        );
        assert!(chat_completions_url("http://evil.example/v1").is_err());
    }

    #[test]
    fn https_requires_a_key_loopback_http_does_not() {
        assert!(ai_commit_requires_api_key(""));
        assert!(ai_commit_requires_api_key("https://api.openai.com/v1"));
        assert!(!ai_commit_requires_api_key("http://127.0.0.1:11434/v1"));
        assert!(!ai_commit_requires_api_key("http://localhost:11434/v1"));
        assert!(!ai_commit_requires_api_key("http://[::1]:11434/v1"));
    }

    #[test]
    fn prompt_contains_only_the_supplied_diff() {
        let prompt = build_commit_suggestion_prompt("diff --git a/a.rs b/a.rs\n+hello\n");
        assert!(prompt.contains("Staged diff:"));
        assert!(prompt.contains("+hello"));
        assert!(!prompt.contains("github_pat_"));
        assert!(!prompt.contains("CLAUDE.md"));
        let body = chat_completions_request_body("gpt-4o-mini", &prompt);
        assert!(body.contains("gpt-4o-mini"));
        assert!(!body.contains("Bearer"));
        assert!(!body.contains("api_key"));
        let default_model = chat_completions_request_body("  ", "prompt");
        assert!(default_model.contains(DEFAULT_AI_COMMIT_MODEL));
        assert!(!default_model.contains("Authorization"));
    }

    #[test]
    fn parses_completion_and_splits_subject_body() {
        let json =
            r#"{"choices":[{"message":{"content":"Fix the parser\n\nHandle empty hunks."}}]}"#;
        let text = parse_chat_completion(json).expect("content");
        let (subject, body) = split_commit_message(&text);
        assert_eq!(subject, "Fix the parser");
        assert_eq!(body, "Handle empty hunks.");
        let (subject_only, empty_body) = split_commit_message("One line subject");
        assert_eq!(subject_only, "One line subject");
        assert!(empty_body.is_empty());
    }

    #[test]
    fn strips_markdown_fences_from_completion() {
        let json =
            r#"{"choices":[{"message":{"content":"```\nShort subject\n\nBody line\n```"}}]}"#;
        let text = parse_chat_completion(json).expect("content");
        let (subject, body) = split_commit_message(&text);
        assert_eq!(subject, "Short subject");
        assert_eq!(body, "Body line");
        let fenced_lang = parse_chat_completion(
            r#"{"choices":[{"message":{"content":"```markdown\nSubject\n\nBody\n```"}}]}"#,
        )
        .expect("fenced");
        let (subject, body) = split_commit_message(&fenced_lang);
        assert_eq!(subject, "Subject");
        assert_eq!(body, "Body");
    }

    #[test]
    fn rejects_empty_or_malformed_completions() {
        assert!(parse_chat_completion("{}").is_err());
        assert!(parse_chat_completion("not json").is_err());
        assert!(parse_chat_completion(r#"{"choices":[{"message":{"content":"   "}}]}"#).is_err());
        assert!(parse_chat_completion(r#"{"choices":[]}"#).is_err());
    }
}
