#![feature(iter_array_chunks)]

use app::App;
use clap::Parser;
use cli::Cli;
use config::Config;

mod app;
mod cli;
mod config;
mod image;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));
    let cli = Cli::parse();
    let config = Config::new(&cli.config);

    let app = App::build(cli.inspect, &config.window);
    app.run();

    Ok(())
}