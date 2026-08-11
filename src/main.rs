use anyhow;
use reqwest::Client;
use scraper;
use scraper::{Html, Selector};
use tokio;
use url::Url;

const URL_STRING: &str =
    "https://kodikplayer.com/serial/73959/68e2e57cb95f7fb93655637acaca26c2/720p";
    // "https://kodikplayer.com/seria/175152/f0155c810cbacd426ed3df86472445c9/720p";


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

    Ok(())
}
