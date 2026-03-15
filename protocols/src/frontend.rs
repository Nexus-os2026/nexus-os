use axum::http::{header, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use include_dir::{include_dir, Dir};

static FRONTEND_DIST: Dir<'_> = include_dir!("$NEXUS_FRONTEND_DIST");
const API_PREFIXES: &[&str] = &[
    "/api", "/a2a", "/mcp", "/auth", "/health", "/metrics", "/ws", "/v1",
];

pub async fn serve_embedded_frontend(uri: Uri) -> Response {
    let path = uri.path();
    if API_PREFIXES
        .iter()
        .any(|prefix| path == *prefix || path.starts_with(&format!("{prefix}/")))
    {
        return (StatusCode::NOT_FOUND, "not found").into_response();
    }

    let requested = path.trim_start_matches('/');
    let asset_path = if requested.is_empty() {
        "index.html"
    } else {
        requested
    };

    if let Some(file) = FRONTEND_DIST.get_file(asset_path) {
        return asset_response(asset_path, file.contents());
    }

    if !asset_path.contains('.') {
        if let Some(index) = FRONTEND_DIST.get_file("index.html") {
            return asset_response("index.html", index.contents());
        }
    }

    (StatusCode::NOT_FOUND, "not found").into_response()
}

fn asset_response(path: &str, contents: &[u8]) -> Response {
    let mime = mime_guess::from_path(path).first_or_octet_stream();
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, mime.as_ref())],
        contents.to_vec(),
    )
        .into_response()
}
