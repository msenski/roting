# rioting

Home security system. Reads RTSP streams from IP cameras and serves them as HLS video in a browser.

## What it does

```
Camera (RTSP) → rioting → ffmpeg → HLS files on disk → axum HTTP server → browser
```

Each camera gets its own ffmpeg process that segments the stream into 2-second `.ts` chunks.
The browser player fetches `/cameras` to discover which cameras are configured, then plays each one.

## Requirements

- Rust (install via [rustup](https://rustup.rs))
- ffmpeg (`sudo apt install ffmpeg`)

## Configuration

Copy and edit `config.toml`:

```toml
server_port = "3000"

[[cameras]]
name = "dining-area"
ip = "192.168.178.x"
user = "your-camera-user"
password = "your-camera-password"

[[cameras]]
name = "living-room"
ip = "192.168.178.x"
user = "your-camera-user"
password = "your-camera-password"
```

`config.toml` is gitignored (it contains credentials).

## Running

```bash
cargo run -- --config-path config.toml
```

Then open `http://localhost:3000` in a browser.
