mod chapters;
mod manifests;
#[cfg(feature = "kodik-api")]
mod search;

pub use manifests::KodikParserClient;
#[cfg(feature = "kodik-api")]
pub use search::GroupReleases;
