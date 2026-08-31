use std::collections::HashMap;

use kodik_api::search::SearchResponse;
use kodik_api::types::{Release, Translation};

pub(super) struct ReleaseGroup<'a>(Vec<&'a Release>);

impl<'a> ReleaseGroup<'a> {
    fn new(release_group: Vec<&'a Release>) -> Self {
        Self(release_group)
    }

    fn title(&self) -> &str {
        self.0[0].title.as_str()
    }
}

pub(super) trait GroupReleases {
    fn group_releases(&self) -> Vec<ReleaseGroup>;
}

impl GroupReleases for SearchResponse {
    fn group_releases(&self) -> Vec<ReleaseGroup> {
        let release_groups = Vec::new();
        for release in &self.results {
            // if compare_releases(r, rel2) {}
        }
        release_groups
    }
}

fn compare_releases(rel1: &Release, rel2: &Release) -> bool {
    if rel1.title == rel2.title {
        return true;
    } else if rel1.title_orig == rel2.title_orig {
        return true;
    }

    false
}
