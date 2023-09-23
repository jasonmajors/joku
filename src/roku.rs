//! Roku commands and ways to send them.

use std::{
    collections::HashMap, fmt::Display, fs, net::SocketAddr, path::PathBuf, str::FromStr,
    time::Duration,
};

use anyhow::{anyhow, bail, Result};
use futures_util::{stream, StreamExt};
use quick_xml::{events::Event, Reader};
use reqwest::{Client, Method, Response, Url};
use serde::{Deserialize, Serialize};
use ssdp_client::SearchTarget;
use structopt::StructOpt;
use tracing::debug;

use crate::urlify;

/// Provides the subcommands to excute the [`External Control API`](https://developer.roku.com/docs/developer-program/debugging/external-control-api.md#keypress-key-values)
#[derive(Debug, StructOpt)]
#[structopt(name = "joku")]
#[derive(Serialize, Deserialize, Clone)]
pub enum RokuCommand {
    /// Discover Roku devices on the network.
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
    /// Search using the Roku External Control Protocol Search API:
    /// [`https://developer.roku.com/docs/developer-program/debugging/external-control-api.md#search-examples`]
    /// TODO: This doesn't seem to work great. Especially the launch flag
    Search(SearchParams),
    /// Launches a Roku app
    Launch(LaunchParams),
    DeviceInfo,
}

/// The params for a search query, however this isn't working great!
#[derive(Debug, StructOpt, Serialize, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub struct SearchParams {
    keyword: String,
    #[structopt(long)]
    r#type: Option<String>,
    #[structopt(long)]
    title: Option<String>,
    #[structopt(long)]
    season: Option<String>,
    #[structopt(long)]
    launch: Option<bool>,
    #[structopt(long)]
    provider: Option<String>,
    #[structopt(long)]
    provider_id: Option<String>,
}

#[derive(Debug, StructOpt, Serialize, Deserialize, Clone)]
pub struct LaunchParams {
    app: RokuApp,
    content_id: String,
}

impl LaunchParams {
    fn path(&self) -> Result<String> {
        let content_id = match self.app {
            RokuApp::YouTube => {
                // Try to parse a URL, if its a URL, take the `v` param
                if let Ok(url) = Url::parse(&self.content_id) {
                    let query: HashMap<_, _> = url.query_pairs().into_iter().collect();
                    let id = query.get("v").map(|v| v.to_string());
                    id
                } else {
                    None
                }
            }
        }
        .ok_or(anyhow!("Invalid content identifier"))?;

        Ok(format!("{}?contentId={content_id}", self.app.id()))
    }
}

// TODO: Maintaining this is untenable.
// To do this the right way, the `discover` command should probably get all of the existing
// apps from `GET /query/apps` and write their metadata to the config.toml.
// And then we can deserialize a `RokuApp` from the toml table.
//
// Instead I'm going to do it the easy way, and hope that the app-id values are universal.
#[derive(Debug, StructOpt, Serialize, Deserialize, Clone)]
#[serde(rename_all = "lowercase")]
enum RokuApp {
    YouTube,
}

impl FromStr for RokuApp {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "youtube" => Ok(RokuApp::YouTube),
            _ => bail!("Unknown Roku app: {}", s),
        }
    }
}

impl RokuApp {
    /// The ID used in the Roku API for a given RokuApp variant.
    /// These appear to all be numerical, but they're just used as IDs
    /// so a String is more convienent.
    fn id(&self) -> String {
        match self {
            Self::YouTube => String::from("837"),
        }
    }
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
            RokuCommand::Search(params) => {
                let base = "search/browse";
                let qs = serde_qs::to_string(&params)
                    .unwrap()
                    .chars()
                    .collect::<String>();

                format!("{base}?{qs}")
            }
            RokuCommand::DeviceInfo => "query/device-info".to_string(),
            RokuCommand::Launch(params) => match params.path() {
                Ok(path) => {
                    format!("launch/{}", path)
                }
                Err(e) => panic!("Bad launch params! {:?}", e),
            },
            // Not a real Roku command, we're using this to discover Roku devices on the network.
            RokuCommand::Discover => "".to_string(),
        };
        write!(f, "{command}")
    }
}

/// Represents the Roku device that commands are sent to.
#[derive(Debug, Serialize, Deserialize)]
pub struct RokuDevice {
    pub name: String,
    pub addr: SocketAddr,
}

impl Display for RokuDevice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}

/// A representation of the config.toml file containing the name and socket address of the Roku device.
#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub device: RokuDevice,
}

/// Encapsulates sending commands to the Roku device
pub struct RokuClient {
    base: Url,
}

impl RokuClient {
    pub fn new(base: Url) -> Self {
        Self { base }
    }

    /// Creates a new `RokuClient`.
    ///
    /// This will read the device's address from the `config.toml`.
    pub fn try_from_config(config: &PathBuf) -> Result<Self> {
        let toml = fs::read(config)?;
        let config: Config = basic_toml::from_slice(&toml)?;

        let url = Url::parse(format!("http://{}", &config.device.addr).as_str())?;

        Ok(Self::new(url))
    }

    /// Sends a `RokuCommand` to the Roku device.
    pub async fn send(&self, command: RokuCommand, method: Method) -> Result<Response> {
        send_cmd(command, &self.base, method).await
    }
}

/// Searches for all Roku devices on the network.
///
/// This will also ping each device to retrieve its "friendly name".
/// TODO: this and its helper fn `get_roku_addr` feel like they should be apart of something...
/// Maybe not `RokuClient`, but something...
pub async fn get_roku_devices() -> Result<Vec<RokuDevice>> {
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

/// Searches for all Roku devices on the network and returns their URLs.
async fn get_roku_addr() -> Result<Vec<Url>> {
    let search_target = SearchTarget::Custom("roku".to_string(), "ecp".to_string());
    let mut responses =
        ssdp_client::search(&search_target, Duration::from_secs(2), 1, None).await?;

    let mut urls = vec![];
    while let Some(response) = responses.next().await {
        let url: Url = response?.location().parse()?;
        urls.push(url);
    }

    Ok(urls)
}

/// Sends a `RokuCommand` to a provided `Url`.
async fn send_cmd(command: RokuCommand, url: &Url, method: Method) -> Result<Response> {
    let url = urlify(url, &command)?;
    debug!(?url, "Sending {:?}", &command);

    let client = Client::new();
    let resp = client.request(method, url).send().await?;

    Ok(resp)
}
