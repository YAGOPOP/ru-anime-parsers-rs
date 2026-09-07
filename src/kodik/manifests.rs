use anyhow::{self, Context};
use base64::{Engine as _, engine::general_purpose};
use regex::Regex;
use scraper;
use scraper::{ElementRef, Html, Selector};
use serde::{Deserialize, Serialize};
use tokio::time::error;
use url::Url;

use std::sync::LazyLock;

use thiserror::Error;

#[derive(Debug, Deserialize)]
pub(super) struct ManifestResponse {
    links: LinksResponse,
}
impl ManifestResponse {
    pub(super) fn links(self) -> LinksResponse {
        self.links
    }
}

#[derive(Debug, Deserialize)]
pub(super) struct LinksResponse {
    #[serde(rename = "360")]
    quality_360p: Vec<RawLink>,
    #[serde(rename = "480")]
    quality_480p: Vec<RawLink>,
    #[serde(rename = "720")]
    quality_720p: Vec<RawLink>,
}

#[derive(Debug, Deserialize)]
struct RawLink {
    #[serde(rename = "src")]
    source: String,
}

impl RawLink {
    fn decrypt(&self, caesar_shift: u8) -> Result<KodikManifestLink, KodikManifestLinkError> {
        if is_manifest_link(&self.source) {
            KodikManifestLink::from_relative(&self.source)
        } else {
            let mut decrypted_caesar = caesar_decrypt(&self.source, caesar_shift);
            pad_base64(&mut decrypted_caesar).ok_or(KodikManifestLinkError::InvalidBase64(
                decrypted_caesar.clone(),
            ))?;
            let decrypted = general_purpose::STANDARD.decode(&decrypted_caesar)?;
            let protocol_rel_url = String::from_utf8(decrypted)?;
            if !is_manifest_link(&protocol_rel_url) {
                return Err(KodikManifestLinkError::InvalidLink(protocol_rel_url));
            }
            Ok(KodikManifestLink::from_relative(&protocol_rel_url)?)
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
    pub fn as_str(&self) -> &str {
        self.manifest_link.as_str()
    }

    fn from_relative(rel_url: &str) -> Result<Self, KodikManifestLinkError> {
        let url = if rel_url.starts_with("//") {
            Url::parse(&format!("https:{}", rel_url))?
        } else {
            Url::parse(rel_url)?
        };
        Ok(KodikManifestLink { manifest_link: url })
    }
}

fn is_manifest_link(value: &str) -> bool {
    value.contains("mp4:hls:manifest")
}

#[derive(Debug)]
pub struct KodikManifestLinks {
    quality_360p: Option<KodikManifestLink>,
    quality_480p: Option<KodikManifestLink>,
    quality_720p: Option<KodikManifestLink>,
}

#[derive(Debug, Error)]
pub enum KodikManifestLinkError {
    #[error("got invalid url")]
    InvalidLink(String),
    #[error("got invalid url")]
    InvalidUrl(#[from] url::ParseError),
    #[error("invalid base64 padding: {0}")]
    InvalidBase64(String),
    #[error("failed to decode manifest link from base64")]
    Base64Decode(#[from] base64::DecodeError),
    #[error("decoded manifest link is not valid UTF-8")]
    InvalidUtf8(#[from] std::string::FromUtf8Error),
}

impl LinksResponse {
    pub(super) fn decrypt(self, caesar_shift: u8) -> KodikManifestLinks {
        let quality_360p = self
            .quality_360p
            .get(0)
            .and_then(|l| l.decrypt(caesar_shift).ok());
        let quality_480p = self
            .quality_480p
            .get(0)
            .and_then(|l| l.decrypt(caesar_shift).ok());
        let quality_720p = self
            .quality_720p
            .get(0)
            .and_then(|l| l.decrypt(caesar_shift).ok());
        KodikManifestLinks {
            quality_360p,
            quality_480p,
            quality_720p,
        }
    }
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

fn pad_base64(value: &mut String) -> Option<()> {
    match value.len() % 4 {
        0 => {}
        2 => value.push_str("=="),
        3 => value.push('='),
        1 => return None,
        _ => unreachable!(),
    }

    Some(())
}
