//! Live end-to-end regression tests that hit the actual production URL.
//!
//! These tests are gated on `E2E_BASE_URL` so CI environments without
//! outbound network simply skip. They exist to assert the fix is actually
//! deployed on the real stack, not just green in unit tests.

use std::time::Duration;

/// Regression test for the room-link preview bug.
///
/// For 7 days `https://oxpulse.chat/{roomId}` returned `404` with no
/// content-type, so Telegram/iMessage link previewers rendered the URL as
/// a "file" instead of a rich card. Commit a0f4a4a made unknown paths
/// fall back to `200 text/html` serving the SvelteKit `index.html`, which
/// ships with full Open Graph meta tags.
#[tokio::test]
async fn room_link_preview_returns_html_not_404() {
    let base_url = match std::env::var("E2E_BASE_URL") {
        Ok(v) if !v.is_empty() => v,
        _ => {
            eprintln!("skipping room_link_preview_returns_html_not_404: E2E_BASE_URL not set");
            return;
        }
    };

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .expect("build reqwest client");

    let url = format!("{}/TQFA-9412", base_url.trim_end_matches('/'));
    let res = client
        .get(&url)
        .send()
        .await
        .unwrap_or_else(|e| panic!("GET {url} failed: {e}"));

    let status = res.status();
    assert_eq!(
        status.as_u16(),
        200,
        "expected 200, got {status} for {url} — SPA fallback regression?"
    );

    let ct = res
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .unwrap_or_else(|| {
            panic!("content-type header missing on {url} — was 404 without type the bug")
        })
        .to_str()
        .expect("content-type is not valid ASCII")
        .to_lowercase();
    assert!(
        ct.starts_with("text/html"),
        "content-type must start with text/html for link previewers, got: {ct}"
    );

    let body = res.text().await.expect("read response body");
    assert!(
        body.contains("property=\"og:title\""),
        "body must contain og:title meta tag for Telegram/iMessage card rendering — \
         not found in {} bytes of body",
        body.len()
    );
    assert!(
        body.contains("property=\"og:image\""),
        "body must contain og:image meta tag for Telegram/iMessage card rendering — \
         not found in {} bytes of body",
        body.len()
    );
}
