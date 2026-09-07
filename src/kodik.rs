mod chapters;
mod manifests;
#[cfg(feature = "kodik-api")]
mod search;
mod client;
mod scripts;
mod page;

pub use client::KodikParserClient;
#[cfg(feature = "kodik-api")]
pub use search::GroupReleases;
