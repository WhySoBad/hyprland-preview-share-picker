use app::App;
use clap::Parser;
use cli::Cli;
use config::Config;
use hyprland_preview_share_picker_lib::toplevel::Toplevel;
use log::LevelFilter;
use schemars::r#gen::SchemaSettings;
use std::io::Write;

mod app;
mod cli;
mod config;
mod image;
mod util;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let config = Config::new(&cli.config);
    let log_file = Box::new(std::fs::File::create(cli.logs).expect("unable to create log file"));
    env_logger::Builder::new()
        .target(env_logger::Target::Pipe(log_file))
        .filter(None, if cli.debug || config.debug { LevelFilter::Debug } else { LevelFilter::Info })
        .format(|buf, record| {
            let now = chrono::Utc::now();
            writeln!(
                buf,
                "[{} {:<5} {}] {}",
                now.format("%Y-%m-%dT%H:%M:%S%.3fZ"),
                record.level(),
                record.target(),
                record.args()
            )
        })
        .init();
    log::debug!("initialized logger");

    match cli.command {
        None => {
            let toplevel_sharing_list = std::env::var("XDPH_WINDOW_SHARING_LIST").unwrap_or_default();
            log::debug!("XDPH_WINDOW_SHARING_LIST = {toplevel_sharing_list}");
            let toplevels = Toplevel::parse(&toplevel_sharing_list);
            log::debug!("using config: {config:#?}");

            log::debug!("got toplevels {toplevels:#?}");

            let app = App::build(cli.inspect, config, toplevels, cli.allow_token_by_default);
            app.run();
        }
        Some(cli::Command::Schema) => {
            let generator = SchemaSettings::draft07().into_generator();
            let schema = generator.into_root_schema_for::<Config>();
            println!("{}", serde_json::to_string_pretty(&schema).expect("should be a valid schema"))
        }
    }

    Ok(())
}
