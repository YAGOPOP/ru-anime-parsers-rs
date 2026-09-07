use scraper;
use scraper::ElementRef;
use serde::Serialize;

use thiserror::Error;

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
    pub title: String,
    pub id: u32,
    pub media_hash: String,
    pub media_id: u32,
    pub media_type: MediaType,
    pub translation_type: PageTranslationType,
    pub selected: bool,
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
pub struct EpisodeInfo {
    pub title: String,
    pub hash: String,
    pub id: u32,
    pub translation_title: Option<String>,
    pub selected: bool,
    pub number: u32,
    pub _other_translation: bool,
}

#[derive(Debug, Error)]
pub enum EpisodeInfoError {
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
