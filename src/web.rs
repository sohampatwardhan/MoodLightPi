use axum::http::{header, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use rust_embed::RustEmbed;

/// The Preact SPA bundle, built by Vite into `web-dist/` and embedded at compile time. The Pi
/// (which has no Node toolchain) compiles this committed directory straight into the binary.
#[derive(RustEmbed)]
#[folder = "web-dist/"]
struct Assets;

/// Catch-all for any request not matched by an explicit `/api/*` or `/ws` route (axum runs this
/// only as the router `fallback`). Routing:
/// - a path under `api/` or exactly `ws` → 404 (an unknown API/WS path must not render the SPA — AUDIT-2)
/// - an exact embedded asset (e.g. `assets/index-<hash>.js`) → that asset, with a cache policy
/// - any other file-like path (has an extension) that is not embedded → 404 (R12.4)
/// - anything else (a client route such as `/mqtt`) → `index.html` so the SPA renders (R12.2)
pub async fn serve_spa(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');

    if path == "ws" || path == "api" || path.starts_with("api/") {
        return StatusCode::NOT_FOUND.into_response();
    }
    if path.is_empty() {
        return serve_index();
    }
    if let Some(content) = Assets::get(path) {
        let mime = mime_guess::from_path(path).first_or_octet_stream();
        // Hashed assets are immutable; everything else (e.g. a root icon) is revalidated.
        let cache = if path.starts_with("assets/") {
            "public, max-age=31536000, immutable"
        } else {
            "no-cache"
        };
        return (
            [
                (header::CONTENT_TYPE, mime.as_ref()),
                (header::CACHE_CONTROL, cache),
            ],
            content.data.into_owned(),
        )
            .into_response();
    }
    // A missing file-like path (has a dot in its final segment) is a genuine 404; a bare client
    // route falls through to the SPA entry document.
    let last = path.rsplit('/').next().unwrap_or(path);
    if last.contains('.') {
        return StatusCode::NOT_FOUND.into_response();
    }
    serve_index()
}

/// Serve `index.html` with `no-cache` so a redeploy is always picked up.
fn serve_index() -> Response {
    match Assets::get("index.html") {
        Some(content) => (
            [
                (header::CONTENT_TYPE, "text/html"),
                (header::CACHE_CONTROL, "no-cache"),
            ],
            content.data.into_owned(),
        )
            .into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embeds_index_and_a_hashed_asset() {
        assert!(Assets::get("index.html").is_some());
        assert!(
            Assets::iter().any(|p| p.starts_with("assets/")),
            "expected at least one built asset under assets/"
        );
    }
}
