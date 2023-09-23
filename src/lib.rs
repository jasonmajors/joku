use std::{env, path::PathBuf};

use anyhow::Result;
use reqwest::Url;
use roku::RokuCommand;

pub mod roku;

fn urlify(base: &Url, command: &RokuCommand) -> anyhow::Result<Url> {
    let url = base.join(&command.to_string())?;

    Ok(url)
}

/// Where the `config.toml` file is located
pub fn config_path() -> Result<PathBuf> {
    let path = PathBuf::from(env::var("HOME")?)
        .join(".config")
        .join("joku");

    Ok(path)
}

fn config_file() -> Result<PathBuf> {
    let file = config_path()?.join("config.toml");

    Ok(file)
}
