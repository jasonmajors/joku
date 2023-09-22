//! A CLI tool for sending Roku commands.
//!
//! I named the command `joku` because my name starts with a J. That's really it.
//!
//! See https://developer.roku.com/docs/developer-program/debugging/external-control-api.md#keypress-key-values
use std::{env, fmt::Display, fs, net::SocketAddr, path::PathBuf};

use anyhow::Result;
use futures_util::{stream, StreamExt};
use inquire::Select;
use quick_xml::{events::Event, Reader};
use reqwest::{Client, Method, Response, Url};
use serde::{Deserialize, Serialize};
use ssdp_client::SearchTarget;
use std::time::Duration;
use structopt::StructOpt;
use tracing::{debug, event, Level};

/// Incomplete. See <https://developer.mozilla.org/en-US/docs/Glossary/Entity#reserved_characters>
const HTML_RESERVED_CHARS: [char; 12] =
    ['?', '[', ']', '@', '#', ':', '<', '>', '&', '"', '-', '_'];

/// Provides the subcommands to excute the [`External Control API`](https://developer.roku.com/docs/developer-program/debugging/external-control-api.md#keypress-key-values)
#[derive(Debug, StructOpt)]
#[structopt(name = "joku")]
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq)]
enum RokuCommand {
    /// Not a real Roku command. We'll use this to discover Roku devices on the network.
    Discover,
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
    DeviceInfo,
}

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
            RokuCommand::DeviceInfo => "query/device-info".to_string(),
            // Not a real Roku command, we're using this to discover Roku devices on the network.
            RokuCommand::Discover => "".to_string(),
        };
        write!(f, "{}", command)
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct RokuDevice {
    name: String,
    addr: SocketAddr,
}

#[derive(Debug, Serialize, Deserialize)]
struct Config {
    device: RokuDevice,
}

impl Display for RokuDevice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt().init();

    let command = RokuCommand::from_args();
    if command == RokuCommand::Discover {
        let devices = get_roku_devices().await?;
        let ans = Select::new("Select your primary Roku device.", devices).prompt()?;

        write_to_config(ans).await?;
    } else {
        // Load the RokuClient and send the command
        let path = config_path()?.join("cargo.toml");

        let toml = fs::read(path)?;
        let config: Config = basic_toml::from_slice(&toml)?;

        let url = Url::parse(format!("http://{}", &config.device.addr).as_str())?;

        send_cmd(command, &url, Method::POST).await?;
    }

    Ok(())
}

fn config_path() -> Result<PathBuf> {
    let path = PathBuf::from(env::var("HOME")?)
        .join(".config")
        .join("joku");

    Ok(path)
}

async fn write_to_config(device: RokuDevice) -> Result<()> {
    let path = config_path()?;
    fs::create_dir_all(path.clone())?;

    let file = path.join("cargo.toml");

    let toml = basic_toml::to_string(&Config { device })?;
    fs::write(file, toml)?;

    Ok(())
}

// TODO: Move this into a struct method? RokuClient?
async fn get_roku_devices() -> Result<Vec<RokuDevice>> {
    let urls = get_roku_addr().await?;
    let device_info_futs = urls
        .iter()
        .map(|url| async move { send_cmd(RokuCommand::DeviceInfo, url, Method::GET).await });

    let mut stream = stream::iter(device_info_futs).buffer_unordered(5);

    let mut devices = vec![];
    // TODO: This is a lot of code to just grab a value out of the XML response.
    // Perhaps we should just parse it manually?
    while let Some(Ok(info)) = stream.next().await {
        let addr = info.remote_addr().unwrap();
        let xml = info.text().await?;
        let mut reader = Reader::from_str(xml.as_str());

        loop {
            match reader.read_event() {
                Ok(Event::Start(e)) if e.name().as_ref() == b"friendly-device-name" => {
                    let name = reader
                        .read_text(e.name())
                        .expect("Cannot decode text value");

                    devices.push(RokuDevice {
                        name: name.to_string(),
                        addr,
                    });
                    break;
                }
                Ok(Event::Eof) => break,
                Err(e) => panic!("Error at position {}: {:?}", reader.buffer_position(), e),
                _ => (),
            }
        }
    }

    Ok(devices)
}

async fn get_roku_addr() -> Result<Vec<Url>> {
    dbg!("Searching for Roku devices...");
    let search_target = SearchTarget::Custom("roku".to_string(), "ecp".to_string());
    let mut responses =
        ssdp_client::search(&search_target, Duration::from_secs(2), 2, None).await?;

    let mut urls = vec![];
    while let Some(response) = responses.next().await {
        let url: Url = response?.location().parse()?;
        urls.push(url);
    }

    Ok(urls)
}

async fn send_cmd(command: RokuCommand, base: &Url, method: Method) -> Result<Response> {
    let url = urlify(base, command.clone())?;
    event!(Level::INFO, ?url, "Sending {:?}", command);

    let client = Client::new();
    let resp = client.request(method, url).send().await?;

    Ok(resp)
}

fn urlify(base: &Url, command: RokuCommand) -> Result<Url> {
    let url = base.join(&command.to_string())?;

    Ok(url)
}
