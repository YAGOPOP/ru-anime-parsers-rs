use kodik_api::search::SearchResponse;
use kodik_api::types::Release;

#[derive(Debug)]
pub struct ReleaseGroup<'a>(Vec<&'a Release>);

pub trait GroupReleases {
    fn group_releases(&self) -> Vec<ReleaseGroup<'_>>;
}

impl<'a> ReleaseGroup<'a> {
    fn new(release_group: Vec<&'a Release>) -> Self {
        Self(release_group)
    }

    fn title(&self) -> &str {
        self.0[0].title.as_str()
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

fn compare_releases(rel1: &Release, rel2: &Release) -> bool {
    if rel1.title == rel2.title {
        return true;
    } else if rel1.title_orig == rel2.title_orig {
        return true;
    }

    false
}

fn update_groups<'a>(groups: &mut Vec<ReleaseGroup<'a>>, release: &'a Release) {
    for group in groups.iter_mut() {
        let Some(first) = group.0.first() else {
            continue;
        };

        if compare_releases(first, release) {
            group.0.push(release);
            return;
        }
    }

    groups.push(ReleaseGroup::new(vec![release]));
}
