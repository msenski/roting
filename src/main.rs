use anyhow::anyhow;
use retina::client::Credentials;
use retina::client::Session;
use retina::client::SessionOptions;
use retina::client::SetupOptions;
use url::Url;

const CAMERA_USER: &str = "CAMERA_USER";
const CAMERA_PASSWORD: &str = "CAMERA_PASSWORD";
const CAMERA_IP: &str = "CAMERA_IP";
const RTSP_PORT: &str = "554";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    let cam_user = dotenvy::var(CAMERA_USER)?;
    let cam_pass = dotenvy::var(CAMERA_PASSWORD)?;
    let cam_ip = dotenvy::var(CAMERA_IP)?;

    let cam_url = Url::parse(format!("rtsp://{cam_ip}:{RTSP_PORT}/stream1").as_str())?;

    let creds = Credentials {
        username: cam_user,
        password: cam_pass,
    };

    let mut session =
        Session::describe(cam_url, SessionOptions::default().creds(Some(creds))).await?;

    println!("{:#?}", session.streams());

    let video_stream_i = session
        .streams()
        .iter()
        .position(|x| x.media() == "video")
        .ok_or_else(|| anyhow!("No video stream found"))?;

    session
        .setup(video_stream_i, SetupOptions::default())
        .await?;

    Ok(())
}
