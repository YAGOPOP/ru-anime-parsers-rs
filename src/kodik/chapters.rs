use crate::kodik::chapters;

use super::manifests::PlayerScript;
use super::manifests::find_js_value;
use regex::Regex;
use std::sync::LazyLock;
use thiserror::Error;

static CHAPTERS_PARSER_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"^(?:\[(?<label>.+)\])?(?<start>[0-9]+(?::[0-9]+){0,2})-(?<end>[0-9]+(?::[0-9]+){0,2})$"#,
    )
    .expect("valid chapters parser regex")
});

#[derive(Debug)]
struct RawChapters<'a> {
    chapters: &'a str,
    media_kind: &'a str,
}

impl PlayerScript {
    fn extract_raw_chapters_info(&self) -> Option<RawChapters<'_>> {
        let (chapters, chapters_media_type) = find_js_value(
            "playerSettings.skipButton = parseSkipButton(\"",
            self.script(),
            "\");",
        )?
        .rsplit_once(",")?;
        let chapters = chapters.trim().strip_suffix('\"')?;
        let media_kind = chapters_media_type.trim().strip_prefix('\"')?;

        Some(RawChapters {
            chapters,
            media_kind,
        })
    }

    /// # Gets chapters from PlayerScript
    /// Returns None when playerSettings.skipButton is not present at all.
    /// For each chapter if chapter_kind is None heuristically tries to determine whether it is opening or endind or etc.
    pub(super) fn get_chapters(&self) -> Option<Vec<Chapter>> {
        let raw_chapters = self.extract_raw_chapters_info()?;
        Some(raw_chapters.parse().into_guessed())
    }
}

#[derive(Debug)]
pub(super) struct Chapter {
    start: u32,
    end: u32,
    kind: Option<ChapterKind>,
}

impl Chapter {
    pub(super) fn start(&self) -> u32 {
        self.start
    }

    pub(super) fn end(&self) -> u32 {
        self.end
    }

    pub(super) fn kind(&self) -> Option<&ChapterKind> {
        self.kind.as_ref()
    }

    fn guess_kind(&mut self, index: usize, total: usize, media_kind: &ChaptersMediaKind) {
        if self.kind.is_some() {
            return;
        }

        if index == 0 && self.start <= 180 {
            self.kind = Some(match media_kind {
                ChaptersMediaKind::Anime => ChapterKind::Opening,
                _ => ChapterKind::Intro,
            });

            return;
        }

        if index == 1 && total == 2 && self.start >= 900 {
            self.kind = Some(match media_kind {
                ChaptersMediaKind::Anime => ChapterKind::Ending,
                _ => ChapterKind::Credits,
            });
        }
    }
}

#[derive(Debug, Error)]
pub(super) enum ChapterParseError {
    #[error("invalid chapter format: {0}")]
    InvalidFormat(String),
    #[error("invalid start: {0}")]
    InvalidStart(String),
    #[error("invalid end: {0}")]
    InvalidEnd(String),
    #[error("start greater than end: start: {0}, end: {1}")]
    InvalidRange(u32, u32),
}

impl TryFrom<&str> for Chapter {
    type Error = ChapterParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let captures = CHAPTERS_PARSER_RE
            .captures(value)
            .ok_or(Self::Error::InvalidFormat(value.to_owned()))?;

        let start = &captures["start"];
        let end = &captures["end"];

        let start = parse_timecode(start).ok_or(Self::Error::InvalidStart(start.to_owned()))?;
        let end = parse_timecode(end).ok_or(Self::Error::InvalidEnd(end.to_owned()))?;

        if start >= end {
            return Err(Self::Error::InvalidRange(start, end));
        }

        let kind = captures
            .name("label")
            .map(|m| ChapterKind::from(m.as_str()));

        Ok(Self { start, end, kind })
    }
}

#[derive(Debug)]
enum ChapterKind {
    Opening,
    Ending,
    Intro,
    Credits,
    Other(String),
}

impl From<&str> for ChapterKind {
    fn from(value: &str) -> Self {
        match value {
            "opening" => Self::Opening,
            "ending" => Self::Ending,
            "intro" => Self::Intro,
            "credits" => Self::Credits,
            o => Self::Other(o.to_owned()),
        }
    }
}

impl<'a> RawChapters<'_> {
    pub(super) fn parse(self) -> Chapters {
        let mut chapters = self
            .chapters
            .split(',')
            .map(str::trim)
            .filter_map(|s| Chapter::try_from(s).ok())
            .collect::<Vec<_>>();
        chapters.sort_by_key(|c| c.start);
        Chapters {
            chapters,
            media_kind: self.media_kind.into(),
        }
    }
}

fn parse_timecode(value: &str) -> Option<u32> {
    let parts: Vec<u32> = value
        .split(':')
        .map(str::parse)
        .collect::<Result<_, _>>()
        .ok()?;

    match parts.as_slice() {
        [s] => Some(*s),
        [m, s] => m.checked_mul(60)?.checked_add(*s),
        [h, m, s] => h
            .checked_mul(3600)?
            .checked_add(m.checked_mul(60)?)?
            .checked_add(*s),
        _ => None,
    }
}

#[derive(Debug)]
enum ChaptersMediaKind {
    Anime,
    Other(String),
}

impl From<&str> for ChaptersMediaKind {
    fn from(value: &str) -> Self {
        match value {
            "anime" => Self::Anime,
            o => Self::Other(o.to_owned()),
        }
    }
}

#[derive(Debug)]
struct Chapters {
    chapters: Vec<Chapter>,
    media_kind: ChaptersMediaKind,
}

impl Chapters {
    pub(super) fn into_guessed(mut self) -> Vec<Chapter> {
        let total = self.chapters.len();

        for (index, chapter) in self.chapters.iter_mut().enumerate() {
            chapter.guess_kind(index, total, &self.media_kind);
        }

        self.chapters
    }
}
