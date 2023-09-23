//! A CLI tool for sending Roku commands.
//!
//! I named the command `joku` because my name starts with a J. That's really it.
//!
//! See https://developer.roku.com/docs/developer-program/debugging/external-control-api.md#keypress-key-values
use std::fs;

use anyhow::Result;
use inquire::Select;
use joku::{
    config_path,
    roku::{self, get_roku_apps, App, Config, RokuClient, RokuCommand, RokuDevice},
};
use reqwest::{Method, Url};
use structopt::StructOpt;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env())
        .init();

    let command = RokuCommand::from_args();

    match command {
        RokuCommand::Discover => {
            println!("Searching for Roku devices...");
            let devices = roku::get_roku_devices().await?;
            let ans = Select::new("Select your primary Roku device.", devices).prompt()?;

            let url = Url::parse(format!("http://{}", ans.addr).as_str())?;
            let apps: Vec<App> = get_roku_apps(&url).await?;

            write_to_config(ans, apps)?;
        }
        _ => {
            let path = config_path()?.join("config.toml");

            let _resp = RokuClient::try_from_config(&path)?
                .send(command, Method::POST)
                .await?;
        }
    }

    Ok(())
}

/// Writes the `RokuDevice` to the `config.toml` file.
/// This will include the name and socket address.
fn write_to_config(device: RokuDevice, apps: Vec<App>) -> Result<()> {
    let path = config_path()?;
    fs::create_dir_all(path.clone())?;

    let file = path.join("config.toml");

    let toml = basic_toml::to_string(&Config { device, apps })?;
    fs::write(file, toml)?;

    Ok(())
}
