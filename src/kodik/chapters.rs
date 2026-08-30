use super::manifests::PlayerScript;
use super::manifests::find_js_value;
use regex::Regex;
use std::sync::LazyLock;

static CHAPTERS_PARSER_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"^(?:\[(.+)\])?([0-9]+(?::[0-9]+)?(?::[0-9]+)?-[0-9]+(?::[0-9]+)?(?::[0-9]+)?)$"#)
        .expect("valid chapters parser regex")
});

#[derive(Debug)]
pub(super) struct RawChaptersInfo<'a> {
    chapters: &'a str,
    chapters_media_type: &'a str,
}

pub(super) fn extract_raw_chapters_info(
    player_script: &PlayerScript,
) -> Option<RawChaptersInfo<'_>> {
    let (chapters, chapters_media_type) = find_js_value(
        "playerSettings.skipButton = parseSkipButton(\"",
        &player_script.0,
        "\");",
    )?
    .rsplit_once(",")?;
    let chapters = chapters.trim().strip_suffix('\"')?;
    let chapters_media_type = chapters_media_type.trim().strip_prefix('\"')?;

    Some(RawChaptersInfo {
        chapters,
        chapters_media_type,
    })
}

#[derive(Debug)]
pub(super) struct ChapterInfo {
    start: u32,
    end: u32,
    title: Option<ChapterKind>,
}

impl ChapterInfo {
    fn parse_chapter(chapter: &str) -> Option<Self> {
        let chapter = CHAPTERS_PARSER_RE.captures(chapter)?;
        let timecodes = chapter.get(2)?.as_str();
        let (start, end) = timecodes.split_once('-')?;
        let start = parse_timecode(start)?;
        let end = parse_timecode(end)?;

        if start >= end {
            return None;
        }

        if let Some(chapter_name) = chapter.get(1) {
            let title = ChapterKind::from(chapter_name.as_str());
            Some(Self {
                start,
                end,
                title: Some(title),
            })
        } else {
            Some(Self {
                start,
                end,
                title: None,
            })
        }
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

impl<'a> RawChaptersInfo<'_> {
    pub(super) fn extract_chapters(self) -> Vec<ChapterInfo> {
        let mut res = Vec::new();
        for chapter in self.chapters.split(',').map(str::trim) {
            let chapter = ChapterInfo::parse_chapter(chapter);
            if let Some(ch) = chapter {
                res.push(ch);
            }
        }
        res
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
