mod kodik;

use anyhow::{self, Context, Ok};
use base64::{Engine as _, engine::general_purpose};
use dotenvy;
use kodik_api::Client as KodikClient;
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
    // dotenvy::dotenv().ok();
    // let kodik_content_url = Url::parse(KODIK_CONTENT_URL).unwrap();

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

    let kodik_parser_client = KodikParserClient::new();

    let url =
        Url::parse("https://kodikplayer.com/serial/73959/68e2e57cb95f7fb93655637acaca26c2/720p")?;

    let link = kodik_parser_client.get_episode_manifest(&url, 5).await?;
    println!("{}", link.as_str());

    let link = kodik_parser_client.get_episode_manifest(&url, 6).await?;
    println!("{}", link.as_str());

    Ok(())
}
