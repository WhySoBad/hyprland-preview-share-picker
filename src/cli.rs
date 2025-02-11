use std::path::Path;

use clap::{Parser, Subcommand};

const CONFIG_PATH: &str = ".config/hyprland-share-picker/config.yaml";
const LOG_PATH: &str = "hyprland-share-picker.log";

#[derive(Parser)]
pub struct Cli {
    #[arg(global = true, long, short, default_value_t = get_default_config_path())]
    /// Alternative path to a config file
    pub config: String,

    #[arg(global = true, long, short)]
    /// Enable debug logs
    pub debug: bool,

    #[arg(global = true, long, short, default_value_t = get_default_logs_path())]
    /// Alternative path to store logs
    pub logs: String,

    #[arg(long, short)]
    /// Start the gtk inspector on application launch
    pub inspect: bool,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand)]
pub enum Command {
    #[clap(hide = true)]
    /// Print the config schema
    Schema,
}

fn get_default_config_path() -> String {
    let home_dir = dirs::home_dir().unwrap_or_default();
    let path = home_dir.join(Path::new(CONFIG_PATH));
    String::from(path.to_str().unwrap_or_default())
}

fn get_default_logs_path() -> String {
    std::env::temp_dir().join(LOG_PATH).to_string_lossy().to_string()
}
