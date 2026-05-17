use std::path::PathBuf;

use serde::Deserialize;
use toml;

#[derive(Clone, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Vendor {
    Tapo,
    Reolink,
}

impl Vendor {
    pub fn default_onvif_port(&self) -> u16 {
        match self {
            Vendor::Tapo => 2020,
            Vendor::Reolink => 8000,
        }
    }

    pub fn rtsp_path(&self) -> &str {
        match self {
            Vendor::Tapo => "stream1",
            Vendor::Reolink => "h264Preview_01_main",
        }
    }
}

#[derive(Clone, Deserialize)]
pub struct CameraConfig {
    pub vendor: Vendor,
    pub name: String,
    pub ip: String,
    pub user: String,
    pub password: String,
    onvif_port: Option<u16>,
}

impl CameraConfig {
    pub fn onvif_port(&self) -> u16 {
        self.onvif_port
            .unwrap_or_else(|| self.vendor.default_onvif_port())
    }
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

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn parses_valid_config() {
        let toml = r#"
            server_port = "3000"
            [[cameras]]
            vendor = "tapo"
            name = "front-door"
            ip = "192.168.1.1"
            user = "admin"
            password = "secret"
        "#;
        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(config.cameras.len(), 1);
        assert_eq!(config.cameras[0].name, "front-door");
    }

    #[test]
    fn parses_multiple_cameras() {
        let toml = r#"                                                                                         
          server_port = "3000"
          [[cameras]]
          vendor = "tapo"
          name = "front-door"
          ip = "192.168.1.1"
          user = "admin"
          password = "secret"
          [[cameras]]
          vendor = "reolink"
          name = "garden"
          ip = "192.168.1.2"
          user = "admin"
          password = "secret"
      "#;
        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(config.cameras.len(), 2);
    }

    #[test]
    fn rejects_unknown_vendor() {
        let toml = r#"                                                                                         
          server_port = "3000"
          [[cameras]]
          vendor = "Tapo"
          name = "front-door"
          ip = "192.168.1.1"
          user = "admin"
          password = "secret"
          [[cameras]]
          vendor = "neolink"
          name = "garden"
          ip = "192.168.1.2"
          user = "admin"
          password = "secret"
      "#;
        assert!(toml::from_str::<Config>(toml).is_err());
    }

    #[test]
    fn rejects_missing_field() {
        let toml = r#"
            [[cameras]]
            name = "front-door"
        "#; // missing vendor, ip, user, password, server_port
        assert!(toml::from_str::<Config>(toml).is_err());
    }
}
