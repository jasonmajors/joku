//! Roku commands and ways to send them.

use std::{collections::HashMap, fmt::Display, fs, net::SocketAddr, path::PathBuf, time::Duration};

use anyhow::{anyhow, bail, Result};
use basic_toml::from_str as toml_from_str;
use futures_util::{stream, StreamExt};
use quick_xml::{events::Event, Reader};
use reqwest::{Client, Method, Response, Url};
use serde::{Deserialize, Serialize};
use serde_xml_rs::from_str;
use ssdp_client::SearchTarget;
use structopt::StructOpt;
use tracing::{debug, error};

use crate::{config_file, urlify};

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
    ListApps,
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
    app: String,
    // TODO: It might be nice if this is optional, and we can just launch apps.
    // In that case, we don't need a `RokuApp` variant for the app, since we don't care about
    // parsing the link.
    link: Option<String>,
}

impl LaunchParams {
    fn path(&self) -> Result<String> {
        // TODO: Separate fn maybe? `load_apps` or something?
        let config = config_file()?;
        let apps: Apps = toml_from_str(&fs::read_to_string(config)?)?;

        let app = apps
            .apps
            .into_iter()
            .find(|a| a.name.to_lowercase() == self.app.to_lowercase())
            .ok_or(anyhow!("Unknown roku app"))?
            .try_into()?;

        let path = match app {
            // TODO: Maintaining this for each app will be very annoying...
            // Perhaps we should have a trait that `RokuApp` implements and move the parsing there.
            RokuApp::YouTube(app_id) => match &self.link {
                Some(url) => {
                    if let Ok(url) = Url::parse(url) {
                        let query: HashMap<_, _> = url.query_pairs().into_iter().collect();
                        // Parse the ID out of the youtube link
                        let id = query
                            .get("v")
                            .map(|v| v.to_string())
                            .map(|content_id| format!("{app_id}?contentId={content_id}"));

                        id
                    } else {
                        None
                    }
                }
                None => Some(app_id),
            },
        }
        .ok_or(anyhow!("Invalid content identifier"))?;

        Ok(path)
    }
}

#[derive(Debug, Clone)]
enum RokuApp {
    /// The YouTube application with its application ID.
    YouTube(String),
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
            // TODO: Would be nice if this also took a callback to send a follow up command, like
            // select/pause/etc
            RokuCommand::Launch(params) => match params.path() {
                Ok(path) => {
                    format!("launch/{}", path)
                }
                Err(e) => panic!("Bad launch params! {:?}", e),
            },
            // Not a real Roku command, we're using this to discover Roku devices on the network.
            RokuCommand::Discover => "".to_string(),
            RokuCommand::ListApps => "query/apps".to_string(),
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
    pub apps: Vec<App>,
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

    pub fn base(&self) -> &Url {
        &self.base
    }
}

// TODO: I hate this being `pub`. Should be just an internal type for parsing.
#[derive(Debug, Deserialize)]
pub struct Apps {
    #[serde(alias = "$value")]
    apps: Vec<App>,
}

// TODO: I hate this being `pub`. Should be just an internal type for parsing.
#[derive(Debug, Serialize, Deserialize)]
pub struct App {
    id: String,
    r#type: String,
    version: String,
    #[serde(alias = "$value")]
    name: String,
}

impl TryFrom<App> for RokuApp {
    type Error = anyhow::Error;

    fn try_from(value: App) -> std::result::Result<Self, Self::Error> {
        match value.name.to_lowercase().as_str() {
            "youtube" => Ok(RokuApp::YouTube(value.id)),
            _ => bail!("Unsupported app: {:?}", value),
        }
    }
}

// TOOD: Make this a method on `Apps`
pub async fn get_roku_apps(base: &Url) -> Result<Vec<App>> {
    let resp = send_cmd(RokuCommand::ListApps, base, Method::GET).await?;

    let body = resp.text().await?;

    let apps: Apps = from_str(&body)?;

    Ok(apps.apps)
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
    // Perhaps we should just parse it manually? But perhaps not
    while let Some(Ok(info)) = stream.next().await {
        let addr = info.remote_addr().unwrap();
        let xml = info.text().await?;
        let mut reader = Reader::from_str(xml.as_str());

        loop {
            match reader.read_event() {
                Ok(Event::Start(e)) if e.name().as_ref() == b"friendly-device-name" => {
                    let name = reader
                        .read_text(e.name())
                        .expect("Cannot decode text value")
                        // Fix the `"` char. There's probably other html chars that need fixing!
                        .replace("&quot;", "\"");

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
    if !resp.status().is_success() {
        error!(?resp, "Request to Roku device failed");
    }

    Ok(resp)
}
