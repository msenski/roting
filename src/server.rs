use std::path::PathBuf;

use crate::config::Config;

use axum::{Json, Router, routing::get};
use tower_http::services::ServeDir;

const STATIC_DIR: &str = "static";
const HLS_DIR: &str = "hls";

/// Builds and runs the HTTP server on port 3000.
///
/// Serves static files from the default static directory, mounts each camera's
/// HLS output under `/hls/<camera_name>`, and exposes `/cameras` as a JSON list
/// of camera names. Each entry in `camera_dirs` is a `(camera_name, dir_path)` pair.
pub async fn serve(config: &Config) -> anyhow::Result<()> {
    let names: Vec<String> = config
        .cameras
        .iter()
        .map(|cam_cfg| cam_cfg.name.clone())
        .collect();
    let mut router = Router::new()
        .route("/cameras", get(move || async move { Json(names) }))
        .fallback_service(ServeDir::new(STATIC_DIR));

    for cam_cfg in config.cameras.iter() {
        let url_path = format!("/hls/{}", cam_cfg.name);
        let fs_path = PathBuf::from(HLS_DIR).join(&cam_cfg.name);
        router = router.nest_service(&url_path, ServeDir::new(fs_path));
    }

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", config.server_port)).await?;
    axum::serve(listener, router).await?;

    Ok(())
}
