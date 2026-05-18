mod camera;
mod config;
mod hls;
mod onvif;
mod server;
mod streamer;

use retina::codec::VideoFrame;
use tokio::sync::mpsc;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use clap::Parser;

use config::Config;
use hls::FFMpegWriter;

use crate::camera::OnvifCamera;
use crate::streamer::Streamer;

const HLS_BASE_PATH: &str = "hls";

#[derive(Parser)]
struct Args {
    #[arg(short, long)]
    config_path: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = Config::load(Args::parse().config_path)?;

    let mut task_set = tokio::task::JoinSet::new();

    let mut cameras: HashMap<String, Arc<OnvifCamera>> = HashMap::new();

    // Setup stream and dedicated ffmpg conversion process for each camera
    for cam_cfg in config.cameras.iter() {
        let (tx, mut rx) = mpsc::channel::<VideoFrame>(100);

        let camera = Arc::new(OnvifCamera::connect(cam_cfg.clone()).await?);
        cameras.insert(cam_cfg.name.clone(), camera.clone());

        let streamer = Streamer::new(
            camera.rtsp_url().clone(),
            cam_cfg.user.clone(),
            cam_cfg.password.clone(),
        );

        task_set.spawn(async move {
            loop {
                match streamer.stream(&tx).await {
                    Ok(()) => break, // stream ended
                    Err(e) => {
                        eprintln!("{e}");
                        tokio::time::sleep(Duration::from_secs(2)).await;
                    }
                }
            }
            Ok(())
        });

        let ffmpeg = FFMpegWriter {
            hls_output_dir: PathBuf::new().join(HLS_BASE_PATH).join(&cam_cfg.name),
        };
        task_set.spawn(async move { ffmpeg.write_hls(&mut rx).await });
    }

    let cameras = Arc::new(cameras);
    task_set.spawn(async move { server::serve(&config, cameras).await });

    if let Some(result) = task_set.join_next().await {
        result??
    };
    Ok(())
}
