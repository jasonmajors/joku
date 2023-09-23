//! A CLI tool for sending Roku commands.
//!
//! I named the command `joku` because my name starts with a J. That's really it.
//!
//! See https://developer.roku.com/docs/developer-program/debugging/external-control-api.md#keypress-key-values
use std::{env, fs, path::PathBuf};

use anyhow::Result;
use inquire::Select;
use joku::roku::{self, Config, RokuClient, RokuCommand, RokuDevice};
use reqwest::Method;
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

            write_to_config(ans)?;
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

/// Where the `config.toml` file is located
fn config_path() -> Result<PathBuf> {
    let path = PathBuf::from(env::var("HOME")?)
        .join(".config")
        .join("joku");

    Ok(path)
}

/// Writes the `RokuDevice` to the `config.toml` file.
/// This will include the name and socket address.
fn write_to_config(device: RokuDevice) -> Result<()> {
    let path = config_path()?;
    fs::create_dir_all(path.clone())?;

    let file = path.join("config.toml");

    let toml = basic_toml::to_string(&Config { device })?;
    fs::write(file, toml)?;

    Ok(())
}
