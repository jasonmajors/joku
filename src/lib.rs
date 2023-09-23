use reqwest::Url;
use roku::RokuCommand;

pub mod roku;

fn urlify(base: &Url, command: &RokuCommand) -> anyhow::Result<Url> {
    let url = base.join(&command.to_string())?;

    Ok(url)
}
