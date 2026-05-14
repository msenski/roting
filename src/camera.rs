// use async_trait::async_trait;

use url::Url;

// #[async_trait]
pub trait Camera {
    /// Returns the RTSP URL used to connect to this camera's video stream.
    fn rtsp_url(&self) -> &Url;

    /// Moves the camera continuously at the given velocity.
    ///
    /// `pan`: -1.0 (full left) to 1.0 (full right), 0.0 = no horizontal movement.
    /// `tilt`: -1.0 (full down) to 1.0 (full up), 0.0 = no vertical movement.
    ///
    /// Movement continues until [`Camera::ptz_stop`] is called.
    async fn ptz_move(&self, pan: f32, tilt: f32) -> anyhow::Result<()>;

    /// Stops all pan/tilt movement immediately.
    async fn ptz_stop(&self) -> anyhow::Result<()>;
}
