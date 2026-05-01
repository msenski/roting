use std::path::PathBuf;

use serde::Deserialize;
use toml;

#[derive(Clone, Deserialize)]
pub struct CameraConfig {
    pub name: String,
    pub ip: String,
    pub user: String,
    pub password: String,
}

#[derive(Clone, Deserialize)]
pub struct Config {
    pub cameras: Vec<CameraConfig>,
    pub server_port: String,
}

impl Config {
    pub fn load(config_path: Option<PathBuf>) -> anyhow::Result<Self> {
        let path = config_path.unwrap_or_else(|| {
            dirs::config_dir().expect(
                "Could not determine system's default config directory. \
       Try providing an explicit path with --config-path (-c)",
            )
        });

        let contents = std::fs::read_to_string(path)?;

        Ok(toml::from_str(&contents)?)
    }
}
