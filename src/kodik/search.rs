use std::borrow::Cow;
use std::fmt::write;

use kodik_api::search::SearchResponse;
use kodik_api::types::Release;

#[derive(Debug)]
pub struct ReleaseGroup<'a>(Vec<&'a Release>);

pub trait GroupReleases {
    fn group_releases(&self) -> Vec<ReleaseGroup<'_>>;
}

trait IsSameRelease {
    fn is_same_release(&self, release: &Release) -> bool;
}

impl IsSameRelease for Release {
    fn is_same_release(&self, release: &Release) -> bool {
        self.title == release.title
            || self.title_orig == release.title_orig
            || compare_options(&self.other_title, &release.other_title)
            || compare_options(&self.kinopoisk_id, &release.kinopoisk_id)
            || compare_options(&self.imdb_id, &release.imdb_id)
            || compare_options(&self.mdl_id, &release.mdl_id)
            || compare_options(&self.worldart_link, &release.worldart_link)
            || compare_options(&self.shikimori_id, &release.shikimori_id)
    }
}

fn compare_options<T>(o1: &Option<T>, o2: &Option<T>) -> bool
where
    T: Eq,
{
    match (o1, o2) {
        (Some(r1), Some(r2)) => r1 == r2,
        _ => false,
    }
}

impl<'a> ReleaseGroup<'a> {
    fn new(release_group: Vec<&'a Release>) -> Self {
        Self(release_group)
    }

    fn title(&self) -> &str {
        self.0[0].title.as_str()
    }

    fn releases(&self) -> &[&Release] {
        self.0.as_slice()
    }
}

impl<'a> std::fmt::Display for ReleaseGroup<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.title())
    }
}

impl GroupReleases for SearchResponse {
    fn group_releases(&self) -> Vec<ReleaseGroup<'_>> {
        let mut release_groups = Vec::new();
        for release in &self.results {
            update_groups(&mut release_groups, release);
        }
        release_groups
    }
}

fn update_groups<'a>(groups: &mut Vec<ReleaseGroup<'a>>, release: &'a Release) {
    for group in groups.iter_mut() {
        let Some(first) = group.0.first() else {
            continue;
        };

        if first.is_same_release(release) {
            group.0.push(release);
            return;
        }
    }

    groups.push(ReleaseGroup::new(vec![release]));
}

pub trait DisplayTitle {
    fn display_title(&self) -> Cow<'_, str>;
}

impl DisplayTitle for Release {
    fn display_title(&self) -> Cow<'_, str> {
        match self.episodes_count {
            Some(c) => Cow::Owned(format!("{} ({} ep.)", self.translation.title, c)),
            None => Cow::Borrowed(self.translation.title.as_ref()),
        }
    }
}
