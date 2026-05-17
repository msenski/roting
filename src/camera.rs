use url::Url;

use crate::config::CameraConfig;
use crate::onvif::OnvifClient;

pub struct OnvifCamera {
    rtsp_url: Url,
    onvif_client: OnvifClient,
}

impl OnvifCamera {
    pub async fn connect(config: CameraConfig) -> anyhow::Result<Self> {
        let rtsp_url = Url::parse(&format!(
            "rtsp://{}:554/{}",
            config.ip,
            config.vendor.rtsp_path()
        ))?;
        let onvif = OnvifClient::connect(config).await?;
        Ok(OnvifCamera {
            rtsp_url,
            onvif_client: onvif,
        })
    }
}

impl OnvifCamera {
    pub fn rtsp_url(&self) -> &Url {
        &self.rtsp_url
    }

    async fn ptz_move(&self, pan: f32, tilt: f32) -> anyhow::Result<()> {
        self.onvif_client.ptz_move(pan, tilt).await
    }

    async fn ptz_stop(&self) -> anyhow::Result<()> {
        self.onvif_client.ptz_stop().await
    }
}
