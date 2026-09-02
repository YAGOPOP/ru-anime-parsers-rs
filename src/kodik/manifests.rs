use anyhow::{self, Context};
use base64::{Engine as _, engine::general_purpose};
use regex::Regex;
use reqwest::Client;
use scraper;
use scraper::{ElementRef, Html, Selector};
use serde::{Deserialize, Serialize};
use url::Url;

use std::collections::HashMap;
use std::sync::LazyLock;

use thiserror::Error;

static CAESAR_SHIFT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"charCodeAt\s*\(\s*0\s*\)\s*\+\s*(\d+)"#).expect("valid caesar shift regex")
});

static KOIDIK_API_BASE_URL: LazyLock<Url> =
    LazyLock::new(|| Url::parse("https://kodikplayer.com").unwrap());

static _SERIAL_TRANSLATIONS_SELECTOR: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse(".serial-translations-box select option").unwrap());
static _MOVIE_TRANSLATIONS_SELECTOR: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse(".movie-translations-box select option").unwrap());
static _EPISODES_SELECTOR: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse(".serial-series-box select option").unwrap());
static SCRIPT_SELECTOR: LazyLock<Selector> = LazyLock::new(|| Selector::parse("script").unwrap());

#[derive(Debug)]
pub(super) struct PlayerScript(String);

#[derive(Debug, Error)]
enum VideoParamsError {
    #[error("vInfo.hash not found")]
    NoHash,
    #[error("vInfo.id not found")]
    NoId,
    #[error("vInfo.type not found")]
    NoType,
}

impl PlayerScript {
    fn new(player_script: String) -> Self {
        Self(player_script)
    }

    pub(super) fn script(&self) -> &str {
        self.0.as_str()
    }

    fn get_video_params(&self) -> Result<VideoParams, VideoParamsError> {
        let mut hash = None;
        let mut id = None;
        let mut video_type = None;

        for (key, value) in self.script().lines().filter_map(parse_vinfo_assignment) {
            match key {
                "hash" => hash = Some(value),
                "id" => id = Some(value),
                "type" => video_type = Some(value),
                _ => {}
            }
        }

        Ok(VideoParams {
            hash: hash.ok_or(VideoParamsError::NoHash)?.to_owned(),
            id: id.ok_or(VideoParamsError::NoId)?.to_owned(),
            video_type: video_type.ok_or(VideoParamsError::NoType)?.to_owned(),
        })
    }
}

impl<'a> From<&ElementRef<'a>> for PlayerScript {
    fn from(value: &ElementRef<'a>) -> Self {
        Self::new(value.inner_html())
    }
}

#[derive(Debug)]
struct SerialScript(String);

#[derive(Debug, Error)]
pub enum SerialScriptError {
    #[error("not found hidden endpoint in serial script")]
    EndpointNotFound,
    #[error("failed to decode endpoint from serial script")]
    EndpointDecodeError(#[from] base64::DecodeError),
    #[error("invalid utf-8 in decoded endpoint")]
    EndpointInvalidUtf8(#[from] std::string::FromUtf8Error),
}

impl SerialScript {
    fn new(script: String) -> Self {
        Self(script)
    }

    /// # Extracts base64 encoded hidden endpoint from script.
    /// The fragment of interest:
    /// ```JavaScript
    /// //...
    /// $.ajax({type:"POST",url:atob("L2Z0b3I="),cache:!1
    /// //...
    /// ```
    fn extract_endpoint(&self) -> Option<&str> {
        self.0
            .split_once("url:atob(\"")?
            .1
            .split_once("\")")
            .map(|(endpoint, _)| endpoint)
    }

    fn get_endpoint(&self) -> Result<String, SerialScriptError> {
        let coded = self
            .extract_endpoint()
            .ok_or(SerialScriptError::EndpointNotFound)?;
        let decoded = general_purpose::STANDARD.decode(coded)?;
        let endpoint = String::from_utf8(decoded)?;
        Ok(endpoint)
    }

    fn extract_caesar_shift(&self) -> Option<u8> {
        let caps = CAESAR_SHIFT_RE.captures(&self.0)?;
        let shift: u8 = caps.get(1)?.as_str().parse().ok()?;

        if shift < 26 { Some(shift) } else { None }
    }
}

#[derive(Debug, Serialize)]
pub enum MediaType {
    Serial(u32),
    Video,
    Other(String),
}

impl MediaType {
    pub fn parse(
        media_type: &str,
        episode_count: Option<&str>,
    ) -> Result<Self, PageTranslationInfoError> {
        match media_type {
            "serial" => {
                let count = episode_count
                    .ok_or(PageTranslationInfoError::NoEpisodeCount)?
                    .parse()
                    .map_err(PageTranslationInfoError::InvalidEpisodeCount)?;

                Ok(Self::Serial(count))
            }
            "video" => Ok(Self::Video),
            other => Ok(Self::Other(other.to_owned())),
        }
    }
}

#[derive(Debug, Serialize)]
pub enum PageTranslationType {
    Voice,
    Subtitles,
    Other(String),
}

impl From<&str> for PageTranslationType {
    fn from(value: &str) -> Self {
        match value {
            "voice" => Self::Voice,
            "subtitles" => Self::Subtitles,
            other => Self::Other(other.to_owned()),
        }
    }
}

#[derive(Debug)]
pub struct PageTranslationInfo {
    title: String,
    id: u32,
    media_hash: String,
    media_id: u32,
    media_type: MediaType,
    translation_type: PageTranslationType,
    selected: bool,
}

#[derive(Debug, Error)]
pub enum PageTranslationInfoError {
    #[error("no data-title attribute in translation tag")]
    NoTitle,
    #[error("invalid episode count")]
    InvalidEpisodeCount(#[source] std::num::ParseIntError),
    #[error("no data-id attribute in translation tag")]
    NoId,
    #[error("invalid translation id")]
    InvalidId(#[source] std::num::ParseIntError),
    #[error("no data-media-hash attribute in translation tag")]
    NoMediaHash,
    #[error("no data-media-id attribute in translation tag")]
    NoMediaId,
    #[error("invalid translation media_id")]
    InvalidMediaId(#[source] std::num::ParseIntError),
    #[error("no data-media-type attribute in translation tag")]
    NoMediaType,
    #[error("no data-episode-count attribute in translation tag but media-type is serial")]
    NoEpisodeCount,
    #[error("no data-translation-type attribute in translation tag")]
    NoPageTranslationType,
    #[error("no value attribute in translation tag")]
    NoValue,
    #[error("invalid translation value")]
    InvalidValue(#[source] std::num::ParseIntError),
    #[error("value and id not match")]
    ValueAndIdNotMatch { value: u32, id: u32 },
}

impl<'a> TryFrom<ElementRef<'a>> for PageTranslationInfo {
    type Error = PageTranslationInfoError;

    fn try_from(element: ElementRef<'a>) -> Result<Self, Self::Error> {
        let value = element.value();

        let title = value.attr("data-title").ok_or(Self::Error::NoTitle)?;

        let id = value
            .attr("data-id")
            .ok_or(Self::Error::NoId)?
            .parse::<u32>()
            .map_err(Self::Error::InvalidId)?;
        let media_hash = value
            .attr("data-media-hash")
            .ok_or(Self::Error::NoMediaHash)?;
        let media_id = value
            .attr("data-media-id")
            .ok_or(Self::Error::NoMediaId)?
            .parse::<u32>()
            .map_err(Self::Error::InvalidMediaId)?;
        let media_type = MediaType::parse(
            value
                .attr("data-media-type")
                .ok_or(Self::Error::NoMediaType)?,
            value.attr("data-episode-count"),
        )?;
        let translation_type = value
            .attr("data-translation-type")
            .ok_or(Self::Error::NoPageTranslationType)?
            .into();
        let selected = value.attr("selected").is_some();
        let value_attr = value
            .attr("value")
            .ok_or(Self::Error::NoValue)?
            .parse::<u32>()
            .map_err(Self::Error::InvalidValue)?;

        if value_attr != id {
            return Err(Self::Error::ValueAndIdNotMatch {
                value: value_attr,
                id,
            });
        }
        Ok(Self {
            title: title.to_owned(),
            id,
            media_hash: media_hash.to_owned(),
            media_id,
            media_type,
            translation_type,
            selected,
        })
    }
}

#[derive(Debug)]
struct EpisodeInfo {
    title: String,
    hash: String,
    id: u32,
    translation_title: Option<String>,
    selected: bool,
    number: u32,
    _other_translation: bool,
}

#[derive(Debug, Error)]
enum EpisodeInfoError {
    #[error("invalid episode id")]
    InvalidId(#[source] std::num::ParseIntError),
    #[error("invalid episode number")]
    InvalidNumber(#[source] std::num::ParseIntError),
    #[error("no data-title attribute in episode tag")]
    NoTitle,
    #[error("no data-hash attribute in episode tag")]
    NoHash,
    #[error("no data-id attribute in episode tag")]
    NoId,
    #[error("no value attribute in episode tag")]
    NoValue,
    #[error("no data-other-translation attribute in episode tag")]
    NoOtherPageTranslation,
}

impl<'a> TryFrom<ElementRef<'a>> for EpisodeInfo {
    type Error = EpisodeInfoError;

    fn try_from(element: ElementRef<'a>) -> Result<Self, Self::Error> {
        let value = element.value();

        let title = value.attr("data-title").ok_or(Self::Error::NoTitle)?;
        let hash = value.attr("data-hash").ok_or(Self::Error::NoHash)?;
        let id = value
            .attr("data-id")
            .ok_or(Self::Error::NoId)?
            .parse::<u32>()
            .map_err(Self::Error::InvalidId)?;
        let translation_title = value.attr("data-translation-title").map(str::to_owned);
        let selected = value.attr("selected").is_some();
        let number = value
            .attr("value")
            .ok_or(Self::Error::NoValue)?
            .parse::<u32>()
            .map_err(Self::Error::InvalidNumber)?;
        let other_translation_text = value
            .attr("data-other-translation")
            .ok_or(Self::Error::NoOtherPageTranslation)?;

        let other_translation = match other_translation_text {
            "true" => true,
            "false" => false,
            _other => false,
        };
        Ok(Self {
            title: title.to_owned(),
            hash: hash.to_owned(),
            id,
            translation_title,
            selected,
            number,
            _other_translation: other_translation,
        })
    }
}

struct UrlParamsScript(String);

#[derive(Debug, Error)]
enum UrlParamsError {
    #[error("urlParams not found")]
    NotFound,
    #[error("")]
    InvalidJson(#[from] serde_json::Error),
}

impl UrlParamsScript {
    fn script(&self) -> &str {
        self.0.as_ref()
    }

    fn new(script: String) -> Self {
        Self(script)
    }

    fn get_url_params(&self) -> Result<UrlParams, UrlParamsError> {
        let url_params_json =
            find_js_var_quoted("urlParams", self.script()).ok_or(UrlParamsError::NotFound)?;
        let url_params = serde_json::from_str(url_params_json)?;
        Ok(url_params)
    }
}

impl<'a> From<&ElementRef<'a>> for UrlParamsScript {
    fn from(value: &ElementRef<'a>) -> Self {
        Self::new(value.inner_html())
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct UrlParams {
    #[serde(rename = "d")]
    domain: String,
    #[serde(rename = "d_sign")]
    domain_sign: String,
    #[serde(rename = "pd")]
    parent_domain: String,
    #[serde(rename = "pd_sign")]
    parent_domain_sign: String,
    #[serde(rename = "ref")]
    referer: String,
    #[serde(rename = "ref_sign")]
    referer_sign: String,
}

#[derive(Debug, Serialize)]
struct VideoParams {
    hash: String,
    id: String,
    #[serde(rename = "type")]
    video_type: String,
}

#[derive(Debug, Serialize)]
struct PostPayload {
    #[serde(flatten)]
    video_params: VideoParams,
    #[serde(flatten)]
    url_params: UrlParams,
    bad_user: &'static str,
    cdn_is_working: &'static str,
}

impl PostPayload {
    fn from_params(url_params: UrlParams, video_params: VideoParams) -> Self {
        Self {
            video_params,
            url_params,
            bad_user: "true",
            cdn_is_working: "true",
        }
    }
}

pub struct KodikParserClient {
    reqwest_client: Client,
}

impl KodikParserClient {
    pub async fn get_episode_manifest(
        &self,
        url: &Url,
        episode_number: u32,
    ) -> anyhow::Result<KodikManifestLink> {
        let client = &self.reqwest_client;

        let querry = format!("?episode={}", episode_number);
        let url = url.join(&querry)?;

        let page_resp = client.get(url.as_str()).send().await?.error_for_status()?;
        let page_resp_text = page_resp.text().await?;
        let document = Html::parse_document(&page_resp_text);

        // let translation_tags = document.select(&_SERIAL_TRANSLATIONS_SELECTOR);
        // let translation = translation_tags
        //     .map(PageTranslationInfo::try_from)
        //     .filter(|r| match r {
        //         Ok(v) => v.selected,
        //         Err(_) => false,
        //     })
        //     .collect::<Result<Vec<_>, _>>()?;

        // if let Some(tr) = translation.get(0) {
        //     if let MediaType::Serial(ep_count) = tr.media_type {
        //         if ep_count < episode_number {
        //             anyhow::bail!(
        //                 "provided episode number excedits amount of episodes in selected translation"
        //             );
        //         }
        //     }
        // }

        let script_tags: Vec<_> = document.select(&SCRIPT_SELECTOR).collect();

        let player_script_tag = script_tags.get(4).context("unable to get player script")?;
        let player_script = PlayerScript::from(player_script_tag);
        let video_params = player_script.get_video_params()?;
        let chapters = player_script.get_chapters();

        let url_params_script_tag = script_tags.get(0).context("url params missing")?;
        let url_params_script = UrlParamsScript::from(url_params_script_tag);
        let url_params: UrlParams = url_params_script.get_url_params()?;

        let serial_script_tag = script_tags.get(1).context("serial_script is None")?;
        let serial_script_source = serial_script_tag
            .value()
            .attr("src")
            .context("src attr missing in serial_script_tag")?;
        let serial_script_url = KOIDIK_API_BASE_URL.join(&serial_script_source)?;
        let serial_script_resp = client
            .get(serial_script_url)
            .send()
            .await?
            .error_for_status()?;
        let serial_script = serial_script_resp
            .text()
            .await
            .context("serial script response has no text")?;
        let serial_script = SerialScript::new(serial_script);

        let endpoint = serial_script.get_endpoint()?;
        let post_url = KOIDIK_API_BASE_URL.join(&endpoint)?;

        let kodik_manifest_response = client
            .post(post_url)
            .form(&PostPayload::from_params(url_params, video_params))
            .send()
            .await?
            .error_for_status()?;

        let kodik_manifest_response = kodik_manifest_response.text().await?;
        let kodik_manifest_response: serde_json::Value =
            serde_json::from_str(&kodik_manifest_response)?;

        let links: RawKodikManifestLinks = serde_json::from_value(kodik_manifest_response)?;
        let link = links
            .get_link_of_quality(360)
            .context("360p source not found")?;

        let caesar_shift = serial_script
            .extract_caesar_shift()
            .context("caesar shift not extracted")?;

        let decrypted_link = link.decrypt(caesar_shift)?;

        Ok(decrypted_link)
    }
    pub fn new() -> Self {
        Self::from_client(Client::new())
    }

    pub fn from_client(reqwest_client: Client) -> Self {
        Self { reqwest_client }
    }

    fn fetch() {}
}

impl Default for KodikParserClient {
    fn default() -> Self {
        Self::new()
    }
}

fn parse_vinfo_assignment(line: &str) -> Option<(&str, &str)> {
    let line = line.trim().strip_prefix("vInfo.")?;
    let (key, value) = line.split_once('=')?;

    let key = key.trim();
    let value = value.trim().strip_prefix('\'')?.strip_suffix("';")?;

    Some((key, value))
}

#[derive(Debug, Deserialize)]
struct RawKodikManifestLink {
    #[serde(rename = "src")]
    source: String,
    #[serde(rename = "type")]
    source_type: String,
}

#[derive(Debug, Deserialize)]
struct RawKodikManifestLinks {
    links: HashMap<String, Vec<RawKodikManifestLink>>,
}

impl RawKodikManifestLinks {
    fn get_link_of_quality(&self, quality: u32) -> Option<&RawKodikManifestLink> {
        let quality_str = quality.to_string();
        self.links.get(&quality_str).and_then(|links| links.first())
    }
}

impl RawKodikManifestLink {
    fn decrypt(&self, caesar_shift: u8) -> anyhow::Result<KodikManifestLink> {
        if is_manifest_link(&self.source) {
            Ok(parse_manifest_url(&self.source)?.into())
        } else {
            let mut decrypted_caesar = caesar_decrypt(&self.source, caesar_shift);
            pad_base64(&mut decrypted_caesar)?;
            let decrypted = general_purpose::STANDARD.decode(&decrypted_caesar)?;
            let protocol_rel_url = String::from_utf8(decrypted)?;
            if !is_manifest_link(&protocol_rel_url) {
                anyhow::bail!(
                    "Obtained invalid in some way manifest link {}",
                    protocol_rel_url
                )
            }
            Ok(parse_manifest_url(&protocol_rel_url)?.into())
        }
    }
}

#[derive(Debug)]
pub struct KodikManifestLink {
    manifest_link: Url,
}

impl From<Url> for KodikManifestLink {
    fn from(url: Url) -> Self {
        Self { manifest_link: url }
    }
}

impl KodikManifestLink {
    pub fn as_str<'a>(&'a self) -> &'a str {
        self.manifest_link.as_str()
    }
}

fn parse_manifest_url(protocol_rel_url: &str) -> anyhow::Result<Url> {
    let url = if protocol_rel_url.starts_with("//") {
        Url::parse(&format!("https:{}", protocol_rel_url))?
    } else {
        Url::parse(protocol_rel_url)?
    };
    Ok(url)
}

fn is_manifest_link(value: &str) -> bool {
    value.contains("mp4:hls:manifest")
}

fn caesar_decrypt(text: &str, shift: u8) -> String {
    text.chars()
        .map(|c| {
            if c.is_ascii_uppercase() {
                let base = b'A';
                let offset = c as u8 - base;
                (base + (offset + shift) % 26) as char
            } else if c.is_ascii_lowercase() {
                let base = b'a';
                let offset = c as u8 - base;
                (base + (offset + shift) % 26) as char
            } else {
                c
            }
        })
        .collect()
}

fn pad_base64(value: &mut String) -> anyhow::Result<()> {
    match value.len() % 4 {
        0 => {}
        2 => value.push_str("=="),
        3 => value.push('='),
        1 => anyhow::bail!("invalid base64 length"),
        _ => unreachable!(),
    }

    Ok(())
}

fn find_js_var<'a>(var_name: &str, source: &'a str) -> Option<&'a str> {
    let prefix = format!("var {} = ", var_name);
    find_js_value(&prefix, source, ";")
}

pub(super) fn find_js_value<'a>(prefix: &str, source: &'a str, suffix: &str) -> Option<&'a str> {
    source.lines().find_map(|line| {
        line.trim()
            .strip_prefix(prefix)
            .and_then(|l| l.strip_suffix(suffix))
    })
}

fn find_js_var_quoted<'a>(var_name: &str, source: &'a str) -> Option<&'a str> {
    find_js_var(var_name, source)?
        .strip_prefix('\'')?
        .strip_suffix('\'')
}
