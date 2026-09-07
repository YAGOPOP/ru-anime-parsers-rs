use base64::{Engine as _, engine::general_purpose};
use serde::{Deserialize, Serialize};
use serde_json;
use thiserror::Error;
use regex::Regex;
use scraper;
use scraper::ElementRef;

use std::sync::LazyLock;


static CAESAR_SHIFT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"charCodeAt\s*\(\s*0\s*\)\s*\+\s*(\d+)"#).expect("valid caesar shift regex")
});


pub(super) struct UrlParamsScript(String);

#[derive(Debug, Deserialize, Serialize)]
pub(super) struct UrlParams {
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

#[derive(Debug, Error)]
pub(super) enum UrlParamsError {
    #[error("urlParams not found")]
    NotFound,
    #[error("invalid urlParams JSON")]
    InvalidJson(#[from] serde_json::Error),
}

impl UrlParamsScript {
    fn script(&self) -> &str {
        self.0.as_ref()
    }

    fn new(script: String) -> Self {
        Self(script)
    }

    pub(super)fn get_url_params(&self) -> Result<UrlParams, UrlParamsError> {
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

#[derive(Debug)]
pub(super) struct PlayerScript(String);

#[derive(Debug, Error)]
pub(super) enum VideoParamsError {
    #[error("vInfo.hash not found")]
    NoHash,
    #[error("vInfo.id not found")]
    NoId,
    #[error("vInfo.type not found")]
    NoType,
}

fn parse_vinfo_assignment(line: &str) -> Option<(&str, &str)> {
    let line = line.trim().strip_prefix("vInfo.")?;
    let (key, value) = line.split_once('=')?;

    let key = key.trim();
    let value = value.trim().strip_prefix('\'')?.strip_suffix("';")?;

    Some((key, value))
}

impl PlayerScript {
    fn new(player_script: String) -> Self {
        Self(player_script)
    }

    pub(super) fn script(&self) -> &str {
        self.0.as_str()
    }

    pub(super) fn get_video_params(&self) -> Result<VideoParams, VideoParamsError> {
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
pub(super) struct SerialScript(String);

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
    pub(super) fn new(script: String) -> Self {
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

    pub(super) fn get_endpoint(&self) -> Result<String, SerialScriptError> {
        let coded = self
            .extract_endpoint()
            .ok_or(SerialScriptError::EndpointNotFound)?;
        let decoded = general_purpose::STANDARD.decode(coded)?;
        let endpoint = String::from_utf8(decoded)?;
        Ok(endpoint)
    }

    pub(super)fn extract_caesar_shift(&self) -> Option<u8> {
        let caps = CAESAR_SHIFT_RE.captures(&self.0)?;
        let shift: u8 = caps.get(1)?.as_str().parse().ok()?;

        if shift < 26 { Some(shift) } else { None }
    }
}

#[derive(Debug, Serialize)]
pub(super) struct VideoParams {
    hash: String,
    id: String,
    #[serde(rename = "type")]
    video_type: String,
}

#[derive(Debug, Serialize)]
pub(super) struct PostPayload {
    #[serde(flatten)]
    video_params: VideoParams,
    #[serde(flatten)]
    url_params: UrlParams,
    bad_user: &'static str,
    cdn_is_working: &'static str,
}

impl PostPayload {
    pub(super) fn from_params(url_params: UrlParams, video_params: VideoParams) -> Self {
        Self {
            video_params,
            url_params,
            bad_user: "true",
            cdn_is_working: "true",
        }
    }
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
