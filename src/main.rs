//! A CLI tool for sending Roku commands.
//!
//! I named the command `joku` because my name starts with a J. That's really it.
//!
//! See https://developer.roku.com/docs/developer-program/debugging/external-control-api.md#keypress-key-values
use std::fmt::Display;

use anyhow::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use structopt::StructOpt;
use tracing::{event, Level};

// TODO: It'd be nice if we didn't have to update this when the IP is reassigned.
const ROKU_DEVICE_IP: &str = "http://192.168.1.3:8060";

/// Provides the subcommands to excute the [`External Control API`](https://developer.roku.com/docs/developer-program/debugging/external-control-api.md#keypress-key-values)
#[derive(Debug, StructOpt)]
#[structopt(name = "joku")]
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
    /// A type to handle the Roku External Control Protocol Search API:
    /// [`https://developer.roku.com/docs/developer-program/debugging/external-control-api.md#search-examples`]
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

/// Incomplete. See <https://developer.mozilla.org/en-US/docs/Glossary/Entity#reserved_characters>
const HTML_RESERVED_CHARS: [char; 12] =
    ['?', '[', ']', '@', '#', ':', '<', '>', '&', '"', '-', '_'];

impl Display for RokuCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let key_cmd = "keypress";
        let command = match self {
            RokuCommand::Pause => format!("{key_cmd}/Pause"),
            RokuCommand::Home => format!("{key_cmd}/Home"),
            RokuCommand::Play => format!("{key_cmd}/Play"),
            RokuCommand::Select => format!("{key_cmd}/Select"),
            RokuCommand::Left => format!("{key_cmd}/Left"),
            RokuCommand::Right => format!("{key_cmd}/Right"),
            RokuCommand::Down => format!("{key_cmd}/Down"),
            RokuCommand::Up => format!("{key_cmd}/Up"),
            RokuCommand::Back => format!("{key_cmd}/Back"),
            RokuCommand::VolumeUp => format!("{key_cmd}/VolumeUp"),
            RokuCommand::VolumeDown => format!("{key_cmd}/VolumeDown"),
            RokuCommand::Mute => format!("{key_cmd}/Mute"),
            RokuCommand::PowerOff => format!("{key_cmd}/PowerOff"),
            RokuCommand::Search { .. } => {
                let base = "search/browser";
                // Creates a querystring
                let qs = serde_qs::to_string(&self)
                    .unwrap()
                    .chars()
                    // Filter out HTML reserved chars, and this is probably missing some.
                    .filter(|c| !HTML_RESERVED_CHARS.contains(c))
                    .collect::<String>()
                    .replace("Search", "");

                format!("{base}?{qs}")
            }
        };
        write!(f, "{}", command)
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt().init();

    let cli = RokuCommand::from_args();

    send_cmd(cli).await?;

    Ok(())
}

async fn send_cmd(command: RokuCommand) -> Result<()> {
    event!(Level::INFO, "Sending {:?}", command);
    let client = Client::new();
    let resp = client.post(urlify(command)).send().await?;

    if resp.status().is_success() {
        event!(Level::INFO, "Done");
    } else {
        event!(Level::ERROR, "Command failed: {:?}", resp);
    }

    Ok(())
}

fn urlify(command: RokuCommand) -> String {
    format!("{ROKU_DEVICE_IP}/{command}")
}
