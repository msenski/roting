mod camera;
mod hls;
mod server;

use anyhow::anyhow;

use retina::codec::VideoFrame;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use std::time::Duration;

use camera::Camera;
use hls::FFMpegWriter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let (tx, mut rx) = mpsc::channel::<VideoFrame>(100);

    let camera = Camera::new()?;

    // extract before moving camera to async context
    let camera_name = camera.name.to_owned();
    let camera_output_dir = camera.hls_output_path.to_path_buf();

    let stream = tokio::spawn(async move {
        loop {
            match camera.stream(&tx).await {
                Ok(()) => break, // stream ended
                Err(e) => {
                    eprintln!("{e}");
                    tokio::time::sleep(Duration::from_secs(2)).await;
                }
            }
        }
    });

    let ffmpeg = FFMpegWriter {
        hls_output_dir: camera_output_dir.clone(),
    };
    let converter: JoinHandle<anyhow::Result<()>> = tokio::spawn(async move {
        match ffmpeg.write_hls(&mut rx).await {
            Ok(()) => {}
            Err(e) => Err(anyhow!("Encountered error while writing to FFMPEG: {e}"))?,
        }
        Ok(())
    });

    let server: JoinHandle<anyhow::Result<()>> = tokio::spawn(async move {
        match server::serve(&[(camera_name, camera_output_dir)]).await {
            Ok(()) => {}
            Err(e) => Err(anyhow!("Encountered error while serving: {e}"))?,
        }
        Ok(())
    });

    let _ = tokio::join!(stream, converter, server);
    Ok(())
}
