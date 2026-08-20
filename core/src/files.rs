//! File upload (`POST /upload`) and signed file URLs.
//!
//! Sharkcord serves files over plain HTTP at `/public/<name>`; uploads
//! go through a dedicated `POST /upload` endpoint authenticated by the
//! `x-token` header (not the WebSocket). Upload returns a temp file id
//! that is later referenced by `messages.send({ files: [tempId] })`,
//! `users.changeAvatar`, etc. When the server has
//! `storageSignedUrlsEnabled`, file URLs must carry an `accessToken`
//! (and its expiry) appended as query params.
//!
//! Header names come from the upstream `UploadHeaders` enum
//! (`x-file-name`, `x-file-type`, `content-length`, `x-token`).

use crate::error::CoreResult;
use serde::{Deserialize, Serialize};

/// Header names used by `POST /upload`. Matches the upstream
/// `UploadHeaders` enum in `packages/shared/src/statics/permissions.ts`.
pub mod headers {
    pub const ORIGINAL_NAME: &str = "x-file-name";
    pub const TYPE: &str = "x-file-type";
    pub const CONTENT_LENGTH: &str = "content-length";
    pub const TOKEN: &str = "x-token";
}

/// A freshly-uploaded temporary file, as returned by `POST /upload`.
/// Maps to `TTempFile` upstream.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TempFile {
    pub id: String,
    pub original_name: String,
    pub size: i64,
    pub md5: String,
    pub path: String,
    pub extension: String,
    pub user_id: i64,
}

/// Upload a file to `POST {origin}/upload`.
///
/// `token` is the JWT (sent in the `x-token` header). Returns the temp
/// file id to pass to `messages.send` / `users.changeAvatar` / etc.
pub async fn upload(
    origin: &str,
    token: &str,
    file_name: &str,
    mime_type: &str,
    bytes: Vec<u8>,
) -> CoreResult<TempFile> {
    let client = reqwest::Client::new();
    let url = format!("{}/upload", origin.trim_end_matches('/'));
    let resp = client
        .post(&url)
        .header(headers::TOKEN, token)
        .header(headers::ORIGINAL_NAME, file_name)
        .header(headers::TYPE, mime_type)
        .header(headers::CONTENT_LENGTH, bytes.len())
        .body(bytes)
        .send()
        .await
        .map_err(|e| crate::CoreError::Other(format!("upload request: {e}")))?;

    match resp.status().as_u16() {
        200 => resp
            .json::<TempFile>()
            .await
            .map_err(|e| crate::CoreError::Other(format!("decode upload response: {e}"))),
        code => {
            let body = resp.text().await.unwrap_or_default();
            Err(crate::CoreError::Other(format!(
                "upload failed: HTTP {code}: {body}"
            )))
        }
    }
}

/// Build the URL for a stored file. When the server has signed URLs
/// enabled, append `?accessToken=...&expires=...`.
#[must_use]
pub fn file_url(
    origin: &str,
    name: &str,
    access_token: Option<&str>,
    expires: Option<i64>,
) -> String {
    let base = format!("{}/public/{}", origin.trim_end_matches('/'), name);
    match (access_token, expires) {
        (Some(tok), Some(exp)) => format!("{base}?accessToken={tok}&expires={exp}"),
        (Some(tok), None) => format!("{base}?accessToken={tok}"),
        _ => base,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_url_plain() {
        assert_eq!(
            file_url("http://h:4991", "abc.png", None, None),
            "http://h:4991/public/abc.png"
        );
    }

    #[test]
    fn file_url_signed() {
        assert_eq!(
            file_url("http://h:4991", "abc.png", Some("tok"), Some(123)),
            "http://h:4991/public/abc.png?accessToken=tok&expires=123"
        );
    }

    #[test]
    fn file_url_trims_trailing_slash() {
        assert_eq!(
            file_url("http://h:4991/", "x", None, None),
            "http://h:4991/public/x"
        );
    }
}
