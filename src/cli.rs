use std::path::Path;

use clap::Parser;

const CONFIG_PATH: &str = ".config/hyprland-screen-picker/config.toml";

#[derive(Parser)]
pub struct Cli {
    #[arg(long, short)]
    /// Start the gtk inspector on application launch
    pub inspect: bool,

    #[arg(long, short, default_value_t = get_default_config_path())]
    /// Alternative path to a config file
    pub config: String,

    /// List of windows which should be available for sharing
    pub window_sharing_list: String,
}

fn get_default_config_path() -> String {
    let home_dir = dirs::home_dir().unwrap_or_default();
    let path = home_dir.join(Path::new(CONFIG_PATH));
    String::from(path.to_str().unwrap_or_default())
}
