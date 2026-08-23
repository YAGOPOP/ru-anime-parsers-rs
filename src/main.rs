mod kodik;

use anyhow::{self, Context, Ok};
use base64::{Engine as _, engine::general_purpose};
use dotenvy;
use kodik_api::Client as KodikAPIClient;
use kodik_api::search::SearchQuery as KodikSearchQuerry;
use kodik_api::types::Episode;
use regex::Regex;
use reqwest::Client;
use scraper;
use scraper::{ElementRef, Html, Selector};
use serde::{Deserialize, Serialize};
use tokio;
use url::Url;

use std::any::Any;
use std::collections::HashMap;
use std::path::Prefix;
use std::sync::LazyLock;

use crate::kodik::KodikParserClient;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    let kodik_api_client = KodikAPIClient::new(std::env::var("KODIK_API_TOKEN")?);


    let mut kodik_search_querry = kodik_api::search::SearchQuery::new();
    let kodik_search_resp = kodik_search_querry.with_title("сто девушек 3").execute(&kodik_api_client).await?;

    dbg!(&kodik_search_resp);

    let release = kodik_search_resp.results.get(0).context("no results")?;

    let mut kodik_player_querry_empty = kodik_api::player::PlayerQuery::new();

    // println!("{}", &release.id);
    println!("{:#?}", &release);

    let kodik_player_resp = kodik_player_querry_empty.with_id(&release.id).execute(&kodik_api_client).await?;
    // return Ok(());

    // let mut kodik_search_querry = KodikSearchQuerry::new();
    // let kodik_api_resp = kodik_search_querry.with_title("re: zero").execute(&kodik_api_client).await?;
    // dbg!(kodik_api_resp);

    // match kodik_content_url
    //     .path_segments()
    //     .context("kodik content url is expected to have path")?
    //     .next()
    //     .context("kodik content url path is expected to be not empty")?
    // {
    //     "serial" => {
    //         let translations = document.select(&SERIAL_TRANSLATIONS_SELECTOR);
    //         let translations = RawTranslationInfo::from_element_refs(translations)?;
    //     }
    //     "seria" | "video" => {
    //         let translations = document.select(&MOVIE_TRANSLATIONS_SELECTOR);
    //         let translations = RawTranslationInfo::from_element_refs(translations)?;
    //     }
    //     v => eprintln!("Unexpected vInfo.type value: `{v}`"),
    // };
    //
    let link = kodik_player_resp.link.context("no player link in response")?;

    let kodik_parser_client = KodikParserClient::new();


    println!("{}", link.as_str());
    let url =
        Url::parse(&format!("https:{}", link))?;
    // let url =
    //     Url::parse("https://kodikplayer.com/serial/76639/608923d1e204b9a4fe988e3bb2ca6b87/720p")?;



    let link = kodik_parser_client.get_episode_manifest(&url, 5).await?;
    println!("{}", link.as_str());

    // let link = kodik_parser_client.get_episode_manifest(&url, 6).await?;
    // println!("{}", link.as_str());

    Ok(())
}
