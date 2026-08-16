use axum::http::{header, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "web/"]
struct Assets;

pub async fn serve_index() -> impl IntoResponse {
    serve_path("index.html")
}

pub async fn serve_asset(uri: Uri) -> impl IntoResponse {
    serve_path(uri.path().trim_start_matches('/'))
}

fn serve_path(path: &str) -> Response {
    match Assets::get(path) {
        Some(content) => {
            let mime = mime_guess::from_path(path).first_or_octet_stream();
            (
                [(header::CONTENT_TYPE, mime.as_ref())],
                content.data.into_owned(),
            )
                .into_response()
        }
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn embeds_the_three_web_assets() {
        assert!(Assets::get("index.html").is_some());
        assert!(Assets::get("style.css").is_some());
        assert!(Assets::get("app.js").is_some());
    }
}
