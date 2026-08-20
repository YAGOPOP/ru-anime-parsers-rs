use anyhow::{self, Context, Ok};
use base64::{Engine as _, engine::general_purpose};
use regex::Regex;
use reqwest::Client;
use scraper;
use scraper::{ElementRef, Html, Selector};
use serde::{Deserialize, Serialize};
use url::Url;

use std::collections::HashMap;
use std::sync::LazyLock;

static CAESAR_SHIFT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"charCodeAt\s*\(\s*0\s*\)\s*\+\s*(\d+)"#).expect("valid caesar shift regex")
});

static KOIDIK_API_BASE_URL: LazyLock<Url> =
    LazyLock::new(|| Url::parse("https://kodikplayer.com").unwrap());

// static SERIAL_TRANSLATIONS_SELECTOR: LazyLock<Selector> =
//     LazyLock::new(|| Selector::parse(".serial-translations-box select option").unwrap());
// static MOVIE_TRANSLATIONS_SELECTOR: LazyLock<Selector> =
//     LazyLock::new(|| Selector::parse(".movie-translations-box select option").unwrap());
// static EPISODES_SELECTOR: LazyLock<Selector> =
//     LazyLock::new(|| Selector::parse(".serial-series-box select option").unwrap());
static SCRIPT_SELECTOR: LazyLock<Selector> = LazyLock::new(|| Selector::parse("script").unwrap());

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
enum MediaType {
    Serial,
    Video,
    Other(String),
}

#[derive(Debug, Serialize)]
enum TranslationType {
    Voice,
    Subtitles,
}

#[derive(Debug, Deserialize)]
struct RawTranslationInfo {
    #[serde(rename = "data-episode-count")]
    episode_count: Option<String>, //u32
    #[serde(rename = "data-id")]
    id: String, //u32
    #[serde(rename = "data-media-hash")]
    media_hash: String,
    #[serde(rename = "data-media-id")]
    media_id: String, //u32
    #[serde(rename = "data-media-type")]
    media_type: String, //MediaType
    #[serde(rename = "data-title")]
    title: String,
    #[serde(rename = "data-translation-type")]
    translation_type: String, //TranslationType
    value: String, //u32; == id
}

impl RawTranslationInfo {
    fn from_element_ref(el_ref: ElementRef) -> anyhow::Result<Self> {
        let el_ref: HashMap<_, _> = el_ref.value().attrs().collect();
        let el_ref = serde_json::to_value(el_ref)?;
        let el_ref: RawTranslationInfo = serde_json::from_value(el_ref)?;
        Ok(el_ref)
    }

    fn from_element_refs<'a>(
        el_refs: impl Iterator<Item = ElementRef<'a>>,
    ) -> anyhow::Result<Vec<Self>> {
        el_refs
            .map(|el| Self::from_element_ref(el))
            .collect::<Result<Vec<_>, _>>()
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawEpisodeInfo {
    #[serde(rename = "data-hash")]
    hash: String,
    #[serde(rename = "data-other-translation")]
    other_translation: String, //bool
    #[serde(rename = "data-id")]
    id: String, //u32
    #[serde(rename = "data-title")]
    title: String,
    #[serde(rename = "data-translation-title")]
    translation_title: Option<String>,
    #[serde(rename = "selected")]
    selected: Option<String>, //bool
    #[serde(rename = "value")]
    number: String, //bool
}

impl RawEpisodeInfo {
    fn from_element_ref(el_ref: ElementRef) -> anyhow::Result<Self> {
        let el_ref: HashMap<_, _> = el_ref.value().attrs().collect();
        let el_ref = serde_json::to_value(el_ref)?;
        let el_ref: RawEpisodeInfo = serde_json::from_value(el_ref)?;
        Ok(el_ref)
    }

    fn from_element_refs<'a>(
        el_refs: impl Iterator<Item = ElementRef<'a>>,
    ) -> anyhow::Result<Vec<Self>> {
        el_refs
            .map(|el| Self::from_element_ref(el))
            .collect::<Result<Vec<_>, _>>()
    }
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

        let script_tags: Vec<_> = document.select(&SCRIPT_SELECTOR).collect();

        let player_script_tag = script_tags.get(4).context("unable to get player script")?;
        let player_script_tag_text = player_script_tag.inner_html();
        let video_params = VideoParams::from_script(&player_script_tag_text)?;

        let url_params_script = script_tags
            .get(0)
            .context("url params missing")?
            .inner_html();
        let url_params_json =
            find_js_var_quoted("urlParams", &url_params_script).context("urlParams not found")?;
        let url_params: UrlParams = serde_json::from_str(url_params_json)?;

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

        let coded_endpoint =
            extract_endpoint(&serial_script).context("unable to extract hidden endpoint")?;
        let decoded = general_purpose::STANDARD.decode(coded_endpoint)?;
        let endpoint = String::from_utf8(decoded)?;
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

        let caesar_shift =
            extract_caesar_shift(&serial_script).context("caesar shift not extracted")?;

        let decrypted_link = link.decrypt(caesar_shift)?;

        // let episodes = document.select(&EPISODES_SELECTOR);
        // let episodes = RawEpisodeInfo::from_element_refs(episodes)?;

        Ok(decrypted_link)
    }
    pub fn new() -> Self {
        Self {
            reqwest_client: Client::new(),
        }
    }
}

/// # Extracts base64 encoded hidden endpoint from script.
/// The fragment of interest:
/// ```JavaScript
/// //...
/// $.ajax({type:"POST",url:atob("L2Z0b3I="),cache:!1
/// //...
/// ```
fn extract_endpoint(data: &str) -> Option<&str> {
    data.split_once("url:atob(\"")?
        .1
        .split_once("\")")
        .map(|(endpoint, _)| endpoint)
}

fn extract_caesar_shift(js: &str) -> Option<u8> {
    let caps = CAESAR_SHIFT_RE.captures(js)?;
    let shift: u8 = caps.get(1)?.as_str().parse().ok()?;

    if shift < 26 { Some(shift) } else { None }
}

fn parse_vinfo_assignment(line: &str) -> Option<(&str, &str)> {
    let line = line.trim().strip_prefix("vInfo.")?;
    let (key, value) = line.split_once('=')?;

    let key = key.trim();
    let value = value.trim().strip_prefix('\'')?.strip_suffix("';")?;

    Some((key, value))
}

impl VideoParams {
    fn from_script(script: &str) -> anyhow::Result<Self> {
        let mut hash = None;
        let mut id = None;
        let mut video_type = None;

        for (key, value) in script.lines().filter_map(parse_vinfo_assignment) {
            match key {
                "hash" => hash = Some(value),
                "id" => id = Some(value),
                "type" => video_type = Some(value),
                _ => {}
            }
        }

        Ok(Self {
            hash: hash.context("vInfo.hash not found")?.into(),
            id: id.context("vInfo.id not found")?.into(),
            video_type: video_type.context("vInfo.type not found")?.into(),
        })
    }
}

fn find_js_var<'a>(var_name: &str, source: &'a str) -> Option<&'a str> {
    let prefix = format!("var {} = ", var_name);
    source.lines().find_map(|line| {
        line.trim()
            .strip_prefix(&prefix)
            .and_then(|l| l.strip_suffix(';'))
    })
}

fn find_js_var_quoted<'a>(var_name: &str, source: &'a str) -> Option<&'a str> {
    find_js_var(var_name, source)?
        .strip_prefix('\'')?
        .strip_suffix('\'')
}

#[derive(Debug, Deserialize)]
struct RawKodikManifestLink {
    #[serde(rename = "src")]
    source: String,
    #[serde(rename = "type")]
    _source_type: String,
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
