#![feature(iter_array_chunks)]

use app::App;
use clap::Parser;
use cli::Cli;
use config::Config;
use log::LevelFilter;
use toplevel::Toplevel;

mod app;
mod cli;
mod config;
mod image;
mod toplevel;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let log_file = Box::new(std::fs::File::create(cli.logs).expect("unable to create log file"));
    env_logger::Builder::new()
        .target(env_logger::Target::Pipe(log_file))
        .filter(None, if cli.debug { LevelFilter::Debug } else { LevelFilter::Info })
        .init();

    let config = Config::new(&cli.config);
    let toplevel_sharing_list = std::env::var("XDPH_WINDOW_SHARING_LIST").unwrap_or_default();
    let toplevels = Toplevel::parse(&toplevel_sharing_list);

    log::debug!("got toplevels {toplevels:#?}");

    let app = App::build(cli.inspect, config, toplevels);
    app.run();

    Ok(())
}
