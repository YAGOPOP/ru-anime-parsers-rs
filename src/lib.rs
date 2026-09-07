mod kodik;

pub use crate::kodik::{
    KodikManifestLink, KodikManifestLinkError, KodikManifestLinks, KodikParserClient,
    KodikParserClientError,
};
pub use kodik::{
    Chapter, ChapterKind, ChapterParseError, Chapters, ChaptersMediaKind, EpisodeInfo,
    EpisodeInfoError, MediaType, PageTranslationInfo, PageTranslationType,
};

#[cfg(feature = "kodik-api")]
pub use crate::kodik::{DisplayTitle, GroupReleases};
