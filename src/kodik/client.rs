use anyhow::{self, Context};
use reqwest::{Client, IntoUrl};
use scraper::{Html, Selector};
use std::sync::LazyLock;
use thiserror::Error;
use url::Url;

use super::manifests::{KodikManifestLinks, LinksResponse, ManifestResponse};
use super::scripts::{PlayerScript, PostPayload, SerialScript, UrlParams, UrlParamsScript};

static KODIK_API_BASE_URL: LazyLock<Url> =
    LazyLock::new(|| Url::parse("https://kodikplayer.com").unwrap());

static _SERIAL_TRANSLATIONS_SELECTOR: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse(".serial-translations-box select option").unwrap());
static _MOVIE_TRANSLATIONS_SELECTOR: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse(".movie-translations-box select option").unwrap());
static _EPISODES_SELECTOR: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse(".serial-series-box select option").unwrap());
static SCRIPT_SELECTOR: LazyLock<Selector> = LazyLock::new(|| Selector::parse("script").unwrap());
const URL_PARAMS_SCRIPT: usize = 0;
const SERIAL_SCRIPT: usize = 1;
const PLAYER_SCRIPT: usize = 4;

pub struct KodikParserClient {
    reqwest_client: Client,
}

#[derive(Debug, Error)]
pub enum KodikParserClientError {
    #[error("Failed request: {0}")]
    RequestFailed(#[from] reqwest::Error),
    #[error("No text in serial script")]
    SerialScriptNoText,
    #[error("unable to parse raw data: {0}")]
    InvalidRawData(#[from] serde_json::Error),
}

impl KodikParserClient {
    pub async fn get_episode_manifest(
        &self,
        url: &Url,
        episode_number: u32,
    ) -> anyhow::Result<KodikManifestLinks> {
        let client = &self.reqwest_client;

        let query = format!("?episode={}", episode_number);
        let url = url.join(&query)?;

        let page_resp = client.get(url.as_str()).send().await?.error_for_status()?;
        let page_resp_text = page_resp.text().await?;
        let document = Html::parse_document(&page_resp_text);

        let script_tags: Vec<_> = document.select(&SCRIPT_SELECTOR).collect();

        let player_script_tag = script_tags
            .get(PLAYER_SCRIPT)
            .context("unable to get player script")?;
        let player_script = PlayerScript::from(player_script_tag);
        let video_params = player_script.get_video_params()?;

        let _chapters = player_script.get_chapters();

        let url_params_script_tag = script_tags
            .get(URL_PARAMS_SCRIPT)
            .context("url params missing")?;
        let url_params_script = UrlParamsScript::from(url_params_script_tag);
        let url_params: UrlParams = url_params_script.get_url_params()?;

        let serial_script_tag = script_tags
            .get(SERIAL_SCRIPT)
            .context("serial_script is None")?;
        let serial_script_source = serial_script_tag
            .value()
            .attr("src")
            .context("src attr missing in serial_script_tag")?;

        let serial_script_url = KODIK_API_BASE_URL.join(serial_script_source)?;
        let serial_script = self.fetch_serial_script(serial_script_url).await?;
        let endpoint = serial_script.get_endpoint()?;
        let post_url = KODIK_API_BASE_URL.join(&endpoint)?;
        let links = self
            .fetch_raw_manifests(
                post_url,
                &PostPayload::from_params(url_params, video_params),
            )
            .await?;

        let caesar_shift = serial_script
            .extract_caesar_shift()
            .context("caesar shift not extracted")?;

        let links = links.decrypt(caesar_shift);

        Ok(links)
    }
    pub fn new() -> Self {
        Self::from_client(Client::new())
    }

    pub fn from_client(reqwest_client: Client) -> Self {
        Self { reqwest_client }
    }

    async fn fetch_serial_script(
        &self,
        serial_script_url: impl IntoUrl,
    ) -> Result<SerialScript, KodikParserClientError> {
        let serial_script_resp = self
            .reqwest_client
            .get(serial_script_url)
            .send()
            .await?
            .error_for_status()?;
        let serial_script = serial_script_resp.text().await?;
        let serial_script = SerialScript::new(serial_script);
        Ok(serial_script)
    }

    async fn fetch_raw_manifests(
        &self,
        url: impl IntoUrl,
        form: &PostPayload,
    ) -> Result<LinksResponse, KodikParserClientError> {
        let links = self
            .reqwest_client
            .post(url)
            .form(form)
            .send()
            .await?
            .error_for_status()?
            .json::<ManifestResponse>()
            .await?
            .links();
        Ok(links)
    }
}

impl Default for KodikParserClient {
    fn default() -> Self {
        Self::new()
    }
}
