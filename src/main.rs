use anyhow;
use reqwest::blocking;
use scraper;

fn main() -> anyhow::Result<()> {
    let client = blocking::Client::new();
    let resp = client
        .get("https://kodikplayer.com/seria/182733/710ab16d42543131d462c132d88ebc27/720p")
        .send();

    let text = resp?.text()?;

    println!("{}", text);

    Ok(())
}
