use anyhow;
use reqwest::Client;
use scraper;
use scraper::{Html, Selector};
use tokio;
use url::Url;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct UrlParams {
    d: String,
    d_sign: String,
    pd: String,
    pd_sign: String,
    r#ref: String,
    ref_sign: String,
    advert_debug: bool,
    first_url: bool,
}


const URL_STRING: &str =
    // "https://kodikplayer.com/serial/73959/68e2e57cb95f7fb93655637acaca26c2/720p";
    "https://kodikplayer.com/seria/175152/f0155c810cbacd426ed3df86472445c9/720p";
    // "https://kodikplayer.com/seria/73993/8cf136c19eef4bc23624d32c0424712e/720p";


#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let url = Url::parse(URL_STRING).unwrap();

    // let client = Client::builder().no_proxy().build().unwrap();
    let client = Client::new();



    let resp = client
        .get(url.as_str())
        .send()
        .await?;

    let resp_text = resp.text().await?;


    let document = Html::parse_document(&resp_text);

    let movie_translations_selector = Selector::parse(".movie-translations-box select option").unwrap();
    let serial_translations_selector = Selector::parse(".serial-translations-box select option").unwrap();
    let episodes_selector = Selector::parse(".serial-series-box select option").unwrap();

    let url_params_js_block_selector = Selector::parse(r#"script[type="text/javascript"]"#).unwrap();

    let serial_translations = document.select(&serial_translations_selector);
    for serial_translation in serial_translations {
        println!("{:#?}", serial_translation);
    }

    let movie_translations = document.select(&movie_translations_selector);
    for movie_translation in movie_translations {
        println!("{:#?}", movie_translation);
    }

    let episodes = document.select(&episodes_selector);
    for episode in episodes {
        println!("{:#?}", episode);
    }

    let mut url_params = document.select(&url_params_js_block_selector);
    let script = url_params.next().unwrap().inner_html();
    let json = script
        .lines()
        .find_map(|line| {
            line.trim()
                .strip_prefix("var urlParams = ")
                .and_then(|s| s.strip_suffix(';'))
                .and_then(|s| s.strip_prefix('\''))
                .and_then(|s| s.strip_suffix('\''))
        })
        .unwrap();

    // let params: serde_json::Value = serde_json::from_str(json)?;
    let params: UrlParams = serde_json::from_str(json)?;


    println!("{params:#?}");

    // std::fs::write("./test.html", &resp_text).unwrap();

    Ok(())
}
