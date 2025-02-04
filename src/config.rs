use std::path::{Path, PathBuf};

use log::{error, warn};
use serde::Deserialize;

#[derive(Deserialize, Default, Debug, Clone)]
pub struct Config {
    #[serde(skip_deserializing)]
    path: PathBuf,
    pub window: WindowConfig,
}

impl Config {
    pub fn new(path_str: &String) -> Self {
        let path = Path::new(path_str);
        if path.exists() {
            let str = std::fs::read_to_string(path).unwrap_or_default();
            match toml::from_str(str.as_str()) {
                Ok(config) => Self { path: path.to_path_buf(), ..config },
                Err(err) => {
                    error!("invalid config file at {path_str}: {err}");
                    std::process::exit(1)
                }
            }
        } else {
            warn!("missing config file at {path_str}, using default instead!");
            Self::default()
        }
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct WindowConfig {
    pub width: i32,
    pub height: i32,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self { width: 1000, height: 500 }
    }
}
