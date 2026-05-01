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

use url::Url;

const RTSP_PORT: &str = "554";

pub struct Camera {
    camera_config: CameraConfig,
}

// TODO add functionality to move camera etc
impl Camera {
    pub fn new(camera_config: CameraConfig) -> Self {
        Camera { camera_config }
    }

    pub async fn stream(&self, tx: &mpsc::Sender<VideoFrame>) -> anyhow::Result<()> {
        let rtsp_url = Url::parse(&format!(
            "rtsp://{}:{RTSP_PORT}/stream1",
            self.camera_config.ip
        ))?;

        let creds = Credentials {
            username: self.camera_config.user.clone(),
            password: self.camera_config.password.clone(),
        };

        let mut session =
            Session::describe(rtsp_url, SessionOptions::default().creds(Some(creds))).await?;

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
