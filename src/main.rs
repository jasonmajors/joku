//! A CLI tool for sending Roku commands.
//!
//! See https://developer.roku.com/docs/developer-program/debugging/external-control-api.md#keypress-key-values

// Home
// Rev
// Fwd
// Play
// Select
// Left
// Right
// Down
// Up
// Back
// InstantReplay
// Info
// Backspace
// Search
// Enter
use anyhow::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::fmt::Display;
use structopt::StructOpt;

const ROKU_DEVICE_IP: &str = "http://192.168.1.2:8060";

#[derive(Debug, StructOpt)]
#[structopt(name = "joku")]

/// Provides the subcommands to excute the [`External Control API`](https://developer.roku.com/docs/developer-program/debugging/external-control-api.md#keypress-key-values)
#[derive(Serialize, Deserialize)]
enum RokuCommand {
    Home,
    Play,
    Pause,
    Select,
    Left,
    Right,
    Down,
    Up,
    Back,
    VolumeUp,
    VolumeDown,
    Mute,
    PowerOff,
    /// A type to handle the options shown here:
    /// https://developer.roku.com/docs/developer-program/debugging/external-control-api.md#search-examples
    #[allow(dead_code)]
    Search {
        keyword: String,
        #[structopt(long)]
        r#type: Option<String>,
        #[structopt(long)]
        title: Option<String>,
        #[structopt(long)]
        season: Option<String>,
        #[structopt(long)]
        launch: Option<bool>,
    },
}
#[derive(Serialize, Deserialize)]
struct Test {
    name: String,
    job: String,
}

impl Display for RokuCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let command = match self {
            RokuCommand::Pause => "keypress/Pause".to_string(),
            RokuCommand::Home => "keypress/Home".to_string(),
            RokuCommand::Play => "keypress/Play".to_string(),
            RokuCommand::Select => "keypress/Select".to_string(),
            RokuCommand::Left => "keypress/Left".to_string(),
            RokuCommand::Right => "keypress/Right".to_string(),
            RokuCommand::Down => "keypress/Down".to_string(),
            RokuCommand::Up => "keypress/Up".to_string(),
            RokuCommand::Back => "keypress/Back".to_string(),
            RokuCommand::VolumeUp => "keypress/VolumeUp".to_string(),
            RokuCommand::VolumeDown => "keypress/VolumeDown".to_string(),
            RokuCommand::Mute => "keypress/Mute".to_string(),
            RokuCommand::PowerOff => "keypress/PowerOff".to_string(),
            RokuCommand::Search { .. } => {
                let base = "search/browser?";
                // jank. has to be a way to just use the serialization on the nested struct
                let qs = serde_qs::to_string(&self)
                    .unwrap()
                    .replace(&['[', ']'], "")
                    .replace("Search", "");

                format!("{}{}", base, qs)
            }
        };
        write!(f, "{}", command)
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = RokuCommand::from_args();

    send_cmd(cli).await?;

    Ok(())
}

async fn send_cmd(command: RokuCommand) -> Result<()> {
    let client = Client::new();

    dbg!(&command);
    let resp = client.post(urlify(command)).body("").send().await?;
    println!("{:#?}", resp);

    Ok(())
}

fn urlify(command: RokuCommand) -> String {
    format!("{}/{}", ROKU_DEVICE_IP, command)
}
