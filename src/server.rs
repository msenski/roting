use std::path::PathBuf;

use axum::{Json, Router, routing::get};
use tower_http::services::ServeDir;

const STATIC_DIR: &str = "static";

/// Builds and runs the HTTP server on port 3000.
///
/// Serves static files from the default static directory, mounts each camera's
/// HLS output under `/hls/<camera_name>`, and exposes `/cameras` as a JSON list
/// of camera names. Each entry in `camera_dirs` is a `(camera_name, dir_path)` pair.
pub async fn serve(camera_dirs: &[(String, PathBuf)]) -> anyhow::Result<()> {
    let names: Vec<String> = camera_dirs.iter().map(|(name, _)| name.clone()).collect();

    let mut router = Router::new()
        .route("/cameras", get(move || async move { Json(names) }))
        .fallback_service(ServeDir::new(STATIC_DIR));

    for (name, dir) in camera_dirs.iter() {
        router = router.nest_service(&format!("/hls/{name}"), ServeDir::new(dir));
    }

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
    axum::serve(listener, router).await?;

    Ok(())
}
