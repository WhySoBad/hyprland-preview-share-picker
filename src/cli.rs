use std::path::Path;

use clap::Parser;

const CONFIG_PATH: &str = ".config/hyprland-share-picker/config.toml";
const LOG_PATH: &str = "hyprland-share-picker.log";

#[derive(Parser)]
pub struct Cli {
    #[arg(long, short)]
    /// Start the gtk inspector on application launch
    pub inspect: bool,

    #[arg(long, short, default_value_t = get_default_config_path())]
    /// Alternative path to a config file
    pub config: String,

    #[arg(long, short)]
    /// Enable debug logs
    pub debug: bool,

    #[arg(long, short, default_value_t = get_default_logs_path())]
    /// Alternative path to store logs
    pub logs: String,
}

fn get_default_config_path() -> String {
    let home_dir = dirs::home_dir().unwrap_or_default();
    let path = home_dir.join(Path::new(CONFIG_PATH));
    String::from(path.to_str().unwrap_or_default())
}

fn get_default_logs_path() -> String {
    std::env::temp_dir().join(LOG_PATH).to_string_lossy().to_string()
}
