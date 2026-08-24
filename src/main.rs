mod kodik;

use anyhow::{self, Context, Ok};
use dotenvy;
use kodik_api;
use tokio;
use url::Url;

use crate::kodik::KodikParserClient;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    let kodik_token = std::env::var("KODIK_API_TOKEN")?;

    let reqwest_client = reqwest::ClientBuilder::new().build()?;
    let kodik_api_client = kodik_api::ClientBuilder::new()
        .api_key(kodik_token)
        .custom_reqwest_client(reqwest_client.clone())
        .build();
    let kodik_parser_client = KodikParserClient::from_client(reqwest_client.clone());

    let kodik_search_resp = kodik_api::search::SearchQuery::new()
        .with_title("re: zero замороженные узы")
        .execute(&kodik_api_client)
        .await?;

    let release = kodik_search_resp.results.get(0).context("no results")?;

    let mut kodik_player_querry_empty = kodik_api::player::PlayerQuery::new();

    let kodik_player_resp = kodik_player_querry_empty
        .with_id(&release.id)
        .execute(&kodik_api_client)
        .await?;

    let link = kodik_player_resp
        .link
        .context("no player link in response")?;

    println!("{}", link.as_str());
    let url = Url::parse(&format!("https:{}", link))?;
    // let url =
    //     Url::parse("https://kodikplayer.com/serial/76639/608923d1e204b9a4fe988e3bb2ca6b87/720p")?;

    let link = kodik_parser_client.get_episode_manifest(&url, 8).await?;
    println!("{}", link.as_str());

    Ok(())
}
