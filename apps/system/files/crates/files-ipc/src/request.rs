use std::sync::Arc;

use files_core::Location;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Reveal {
    Folders(Vec<Location>),
    Items(Vec<Location>),
    Properties(Vec<Location>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Request {
    pub reveal: Reveal,
    pub activation_token: Option<String>,
}

impl Reveal {
    #[must_use]
    pub fn classify(uris: &[String]) -> Option<Self> {
        let targets = parse(uris)?;
        let folders = targets
            .iter()
            .all(|at| at.to_path().is_some_and(|path| path.is_dir()));
        Some(match folders {
            true => Self::Folders(targets),
            false => Self::Items(targets),
        })
    }

    #[must_use]
    pub fn from_uris(uris: &[String], make: fn(Vec<Location>) -> Self) -> Option<Self> {
        parse(uris).map(make)
    }

    #[must_use]
    pub fn destination(&self) -> Option<(Location, Vec<Location>)> {
        match self {
            Self::Folders(targets) => targets.first().map(|at| (at.clone(), Vec::new())),
            Self::Items(targets) | Self::Properties(targets) => {
                let first = targets.first()?;
                let folder = first.parent()?;
                let selected = targets
                    .iter()
                    .filter(|at| at.parent().as_ref() == Some(&folder))
                    .cloned()
                    .collect();
                Some((folder, selected))
            }
        }
    }
}

fn parse(uris: &[String]) -> Option<Vec<Location>> {
    let targets: Vec<Location> = uris.iter().filter_map(|uri| Location::parse(uri)).collect();
    (!targets.is_empty()).then_some(targets)
}

pub type Sink = Arc<dyn Fn(Request) + Send + Sync>;
