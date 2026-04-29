use retina::codec::VideoFrame;
use std::path::PathBuf;
use std::process::Stdio;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::sync::mpsc;

pub struct FFMpegWriter {
    pub hls_output_dir: PathBuf,
}

impl FFMpegWriter {
    pub async fn write_hls(&self, rx: &mut mpsc::Receiver<VideoFrame>) -> anyhow::Result<()> {
        // Create ouput directory for HLS
        std::fs::create_dir_all(&self.hls_output_dir)?;

        let segment_path = format!("{}/output%03d.ts", self.hls_output_dir.display());
        let playlist_path = format!("{}/output.m3u8", self.hls_output_dir.display());

        // We will take the VideoFrames from the demuxed session and pipe them into
        // the ffmpeg command. The command will be spawned as a child process
        let mut ffmpeg_child = Command::new("ffmpeg")
            .args(["-f", "h264"]) // Input format is rad H.264
            // TODO: set this dynamically based on camera info
            .args(["-use_wallclock_as_timestamps", "1"]) // Uses system clock to generate timestamps
            .args(["-i", "pipe:0"]) // read from stdin (pipe number 0)
            .args(["-c", "copy"]) // Don't reencode, repackage (whataver this means)
            .args(["-f", "hls"]) // Output format is HLS
            .args(["-hls_time", "2"]) // 2 second segments
            .args(["-hls_list_size", "5"]) // keep only 5 segments in the playlist
            .args(["-hls_flags", "delete_segments"]) // auto-delete old .ts files so disk doesn't fill up
            .args(["-hls_segment_filename", &segment_path]) // zero-padded segment names (output001.ts instead of output1.ts)
            .args([&playlist_path]) // Write the playlist here
            .stdin(Stdio::piped()) // Create a pipe to the stdin
            .spawn()
            .expect("Failed to spawn child process for ffmpeg");

        // Create a handle to the child's stdin
        let mut ffmpeg_stdin = ffmpeg_child
            .stdin
            .take()
            .ok_or_else(|| anyhow::anyhow!("Failed to get ffmpeg stdin"))?;

        while let Some(frame) = rx.recv().await {
            // When you call write_all(bytes), those bytes don't go directly to ffmpeg. They go into a buffer — a small
            // chunk of memory sitting inside your Rust process. Think of it like a holding tank:
            //   function                     OS / ffmpeg
            //   ─────────────────────         ──────────────
            //   write_all(frame)         →  [buffer: ...bytes...]  →  (not sent yet)
            //   write_all(frame)         →  [buffer: .........more bytes...]  →  (not sent yet)
            //   flush()                  →  [buffer empties]  →  bytes finally arrive at ffmpeg

            // The buffer exists for performance — making a system call to actually send bytes
            // across a pipe is relatively expensive. Buffering batches many
            ffmpeg_stdin.write_all(frame.data()).await?;
            ffmpeg_stdin.flush().await?;
        }

        Ok(())
    }
}
