use std::io;
use std::path::{Path, PathBuf};

use cut_project::{Catalog, Error, Media, MediaId, Project, Relocation};

use crate::probe::{self, ProbeError};

#[derive(Debug)]
pub(crate) enum Refusal {
    Probe(ProbeError),
    Unfit(Error),
    Unlike,
}

#[derive(Debug)]
pub(crate) struct Outcome {
    pub(crate) media: MediaId,
    pub(crate) found: Result<Media, (PathBuf, Refusal)>,
}

#[derive(Debug)]
pub(crate) struct Searched {
    pub(crate) outcomes: Vec<Outcome>,
    pub(crate) unreadable: usize,
}

pub(crate) fn tried(project: &Project, media: MediaId, path: &Path) -> Result<Media, Refusal> {
    let found = probe::footage(path).map_err(Refusal::Probe)?;
    project.relink_fit(media, &found).map_err(Refusal::Unfit)?;
    Ok(found)
}

pub(crate) fn located(
    project: &Project,
    media: MediaId,
    chosen: PathBuf,
    others: &[(MediaId, Media)],
) -> Vec<Outcome> {
    let found = tried(project, media, &chosen);
    let relocation = match (&found, project.media(media)) {
        (Ok(_), Some(original)) => Relocation::between(original.path(), &chosen),
        _ => None,
    };
    let mut outcomes = vec![Outcome {
        media,
        found: found.map_err(|refusal| (chosen, refusal)),
    }];
    let Some(relocation) = relocation else {
        return outcomes;
    };
    for (other, original) in others.iter().filter(|(other, _)| *other != media) {
        let candidates = relocation.candidates(original.path());
        if let Some(Ok(found)) = matched(project, *other, original, candidates) {
            outcomes.push(Outcome {
                media: *other,
                found: Ok(found),
            });
        }
    }
    outcomes
}

pub(crate) fn searched(
    project: &Project,
    root: &Path,
    wanted: &[(MediaId, Media)],
) -> io::Result<Searched> {
    let catalog = Catalog::scan(root, wanted.iter().map(|(_, original)| original.path()))?;
    let outcomes = wanted
        .iter()
        .filter_map(|(media, original)| {
            let found = matched(project, *media, original, catalog.find(original.path()))?;
            Some(Outcome {
                media: *media,
                found,
            })
        })
        .collect();
    Ok(Searched {
        outcomes,
        unreadable: catalog.unreadable(),
    })
}

fn matched(
    project: &Project,
    media: MediaId,
    original: &Media,
    candidates: Vec<PathBuf>,
) -> Option<Result<Media, (PathBuf, Refusal)>> {
    let mut refused = None;
    for path in candidates.into_iter().filter(|path| path.is_file()) {
        match tried(project, media, &path) {
            Ok(found) if found.resembles(original) => return Some(Ok(found)),
            Ok(_) => {
                refused.get_or_insert((path, Refusal::Unlike));
            }
            Err(refusal) => {
                refused.get_or_insert((path, refusal));
            }
        }
    }
    refused.map(Err)
}
