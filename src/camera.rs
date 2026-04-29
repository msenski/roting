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

use std::path::PathBuf;

use url::Url;

const CAMERA_USER: &str = "CAMERA_USER";
const CAMERA_PASSWORD: &str = "CAMERA_PASSWORD";
const CAMERA_IP: &str = "CAMERA_IP";
const RTSP_PORT: &str = "554";

pub struct Camera {
    pub name: String,
    pub ip: String,
    pub hls_output_path: PathBuf,
    user: String,
    password: String,
}

impl Camera {
    pub fn new() -> anyhow::Result<Self> {
        // TODO Create camera from config file once we have more cameras
        dotenvy::dotenv().ok();

        let cam_user = dotenvy::var(CAMERA_USER)?;
        let cam_pass = dotenvy::var(CAMERA_PASSWORD)?;
        let cam_ip = dotenvy::var(CAMERA_IP)?;

        let camera_name = "dining-area".to_string();
        // TODO_CLAUDE: Is the path correctly constructed? And should i clone the name or to_owned()?
        let output_path = PathBuf::new().join("hls").join(&camera_name);

        Ok(Camera {
            name: camera_name,
            ip: cam_ip,
            hls_output_path: output_path,
            user: cam_user,
            password: cam_pass,
        })
    }

    pub async fn stream(&self, tx: &mpsc::Sender<VideoFrame>) -> anyhow::Result<()> {
        let ip = self.ip.clone();
        let cam_url = Url::parse(format!("rtsp://{ip}:{RTSP_PORT}/stream1").as_str())?;

        let creds = Credentials {
            username: self.user.clone(),
            password: self.password.clone(),
        };

        let mut session =
            Session::describe(cam_url, SessionOptions::default().creds(Some(creds))).await?;

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
