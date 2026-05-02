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

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn parses_valid_config() {
        let toml = r#"
            server_port = "3000"
            [[cameras]]
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
          name = "front-door"                                                                                
          ip = "192.168.1.1"
          user = "admin"                                                                                     
          password = "secret"
          [[cameras]]                                                                                        
          name = "garden"
          ip = "192.168.1.2"                                                                                 
          user = "admin"
          password = "secret"
      "#;
        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(config.cameras.len(), 2);
    }

    #[test]
    fn rejects_missing_field() {
        let toml = r#"
            [[cameras]]
            name = "front-door"
        "#; // missing ip, user, password, server_port
        assert!(toml::from_str::<Config>(toml).is_err());
    }
}
