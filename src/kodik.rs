mod chapters;
mod client;
mod manifests;
mod page;
mod scripts;
#[cfg(feature = "kodik-api")]
mod search;

pub use chapters::{Chapter, ChapterKind, ChapterParseError, Chapters, ChaptersMediaKind};
pub use client::{KodikParserClient, KodikParserClientError};
pub use manifests::{KodikManifestLink, KodikManifestLinkError, KodikManifestLinks};
pub use page::{
    EpisodeInfo, EpisodeInfoError, MediaType, PageTranslationInfo, PageTranslationType,
};

#[cfg(feature = "kodik-api")]
pub use search::{DisplayTitle, GroupReleases};
