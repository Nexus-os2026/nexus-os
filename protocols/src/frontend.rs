use axum::body::Body;
use axum::http::{Method, Request, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use std::path::PathBuf;
use tower::ServiceExt;
use tower_http::services::{ServeDir, ServeFile};

const API_PREFIXES: &[&str] = &[
    "/api", "/a2a", "/mcp", "/auth", "/health", "/metrics", "/ws", "/v1",
];

fn frontend_dist_dir() -> PathBuf {
    std::env::var_os("NEXUS_FRONTEND_DIST")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("app/dist"))
}

fn is_api_path(path: &str) -> bool {
    API_PREFIXES
        .iter()
        .any(|prefix| path == *prefix || path.starts_with(&format!("{prefix}/")))
}

pub async fn serve_frontend(method: Method, uri: Uri) -> Response {
    if is_api_path(uri.path()) {
        return (StatusCode::NOT_FOUND, "not found").into_response();
    }

    let dist_dir = frontend_dist_dir();
    let index_path = dist_dir.join("index.html");
    if !index_path.exists() {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            format!("frontend assets not found at {}", dist_dir.display()),
        )
            .into_response();
    }

    let service = ServeDir::new(dist_dir)
        .append_index_html_on_directories(true)
        .fallback(ServeFile::new(index_path));
    let request = Request::builder()
        .method(method)
        .uri(uri)
        .body(Body::empty())
        .expect("frontend request should build");

    match service.oneshot(request).await {
        Ok(response) => response.into_response(),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to serve frontend: {error}"),
        )
            .into_response(),
    }
}
