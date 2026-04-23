use anyhow::anyhow;
use futures::StreamExt;
use retina::client::Credentials;
use retina::client::PlayOptions;
use retina::client::Session;
use retina::client::SessionOptions;
use retina::client::SetupOptions;
use retina::codec::CodecItem;
use retina::codec::FrameFormat;
use url::Url;

use std::io::Write;
use std::process::{Command, Stdio};

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

    // We will take the VideoFrames from the demuxed session and pipe them into
    // the ffmpeg command. The command will be spawned as a child process
    let mut ffmpeg_child = Command::new("ffmpeg")
        .args(["-f", "h264"]) // Input format is rad H.264
        .args(["-i", "pipe:0"]) // read from stdin (pipe number 0)
        .args(["-c", "copy"]) // Don't reencode, repackage (whataver this means)
        .args(["-f", "hls"]) // Output format is HLS
        .args(["output.m3u8"]) // Write the playlist here
        .stdin(Stdio::piped()) // Create a pipe to the stdin
        .spawn()
        .expect("Failed to spawn child process for ffmpeg");

    // Create a handle to the child's stdin
    let mut ffmpeg_stdin = ffmpeg_child.stdin.take().unwrap();

    while let Some(res) = playing_session.next().await {
        match res {
            Ok(CodecItem::VideoFrame(f)) => {
                // When you call write_all(bytes), those bytes don't go directly to ffmpeg. They go into a buffer — a small
                // chunk of memory sitting inside your Rust process. Think of it like a holding tank:
                //   function                     OS / ffmpeg
                //   ─────────────────────         ──────────────
                //   write_all(frame)         →  [buffer: ...bytes...]  →  (not sent yet)
                //   write_all(frame)         →  [buffer: .........more bytes...]  →  (not sent yet)
                //   flush()                  →  [buffer empties]  →  bytes finally arrive at ffmpeg

                // The buffer exists for performance — making a system call to actually send bytes
                // across a pipe is relatively expensive. Buffering batches many small writes into one big send.
                ffmpeg_stdin.write_all(f.data())?;
                ffmpeg_stdin.flush()?;
            }
            Ok(_) => {}
            Err(e) => Err(anyhow!("Encountered error while looping over stream: {e}"))?,
        }
    }

    // let mut ls = Command::new("ls")
    // .stdout(Stdio::piped()).spawn().expect("Could not spawn ls");

    // let grep = Command::new("grep")
    // .args(["Cargo"]).stdin(Stdio::from(ls.stdout.take().unwrap())).spawn().expect("Could not spawn grep");

    Ok(())
}
