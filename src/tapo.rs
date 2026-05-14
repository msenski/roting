use anyhow::anyhow;
use futures::StreamExt;
use retina::client::Credentials;
use retina::client::PlayOptions;
use retina::client::Session;
use retina::client::SessionOptions;
use retina::client::SetupOptions;
use retina::codec::CodecItem;
use retina::codec::FrameFormat;
use retina::codec::VideoFrame;
use tokio::sync::mpsc;

use crate::config::CameraConfig;
use crate::config::Vendor;

use url::Url;

const RTSP_PORT: u16 = 554;

fn rtsp_path(vendor: &Vendor) -> &'static str {
    match vendor {
        Vendor::Tapo => "stream1",
        Vendor::Reolink => "h264Preview_01_main",
    }
}

pub struct Camera {
    camera_config: CameraConfig,
    rtsp_url: Url,
}

// TODO add functionality to move camera etc
impl Camera {
    pub fn new(camera_config: CameraConfig) -> anyhow::Result<Self> {
        let rtsp_url = Url::parse(&format!(
            "rtsp://{}:{RTSP_PORT}/{}",
            camera_config.ip,
            rtsp_path(&camera_config.vendor),
        ))?;

        Ok(Camera {
            camera_config,
            rtsp_url,
        })
    }

    pub async fn stream(&self, tx: &mpsc::Sender<VideoFrame>) -> anyhow::Result<()> {
        let creds = Credentials {
            username: self.camera_config.user.clone(),
            password: self.camera_config.password.clone(),
        };

        let mut session = Session::describe(
            self.rtsp_url.clone(),
            SessionOptions::default().creds(Some(creds)),
        )
        .await?;

        let video_stream_i = session
            .streams()
            .iter()
            .position(|x| x.media() == "video")
            .ok_or_else(|| anyhow!("No video stream found"))?;

        session
            .setup(
                video_stream_i,
                SetupOptions::default().frame_format(FrameFormat::SIMPLE),
            )
            .await?;

        // Now with play, the session will be transformed into a demuxed value.
        //
        // The tapo camera is sending data over the network in a muxed state - it
        // doesn't just send raw video, it sends a "combined" stream of video packets
        // (H.264) frames, audio packets (AAC or PCM data) and some Metadata (timestamps
        // or frame rates etc).
        // Demuxing means "unpacking" those combined values. The retina create takes
        // the work of parsing the bits into their respective forms and gives us a stream
        // of high-level objects (retina::client::PlayItem::{VideoFrame, AudioFrame,...})

        let mut playing_session = session.play(PlayOptions::default()).await?.demuxed()?;

        while let Some(res) = playing_session.next().await {
            match res {
                Ok(CodecItem::VideoFrame(f)) => match tx.try_send(f) {
                    Ok(_) => {}
                    Err(_) => Err(anyhow!("FFMPEG's buffer is full. Dropping frame..."))?,
                },
                Ok(_) => {}
                Err(e) => Err(anyhow!("Encountered error while looping over stream: {e}"))?,
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tapo_uses_stream1() {
        assert_eq!(rtsp_path(&Vendor::Tapo), "stream1");
    }

    #[test]
    fn reolink_uses_h264_main_stream() {
        assert_eq!(rtsp_path(&Vendor::Reolink), "h264Preview_01_main");
    }
}
