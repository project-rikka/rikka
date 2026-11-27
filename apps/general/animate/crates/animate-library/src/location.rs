use std::path::{Path, PathBuf};

use rikka_types::Choice;

use crate::error::{AtPath, Result};
use crate::home;
use crate::shelf::Shelf;

const LIBRARY: &str = "library";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Layout {
    Library,
    Shelf(Shelf),
}

impl Layout {
    #[must_use]
    pub fn slug(self) -> &'static str {
        match self {
            Layout::Library => LIBRARY,
            Layout::Shelf(shelf) => shelf.slug(),
        }
    }

    #[must_use]
    pub fn from_slug(slug: &str) -> Option<Layout> {
        match slug {
            LIBRARY => Some(Layout::Library),
            slug => Shelf::from_slug(slug).map(Layout::Shelf),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Location {
    pub root: PathBuf,
    pub layout: Layout,
}

impl Location {
    #[must_use]
    pub fn library(root: impl Into<PathBuf>) -> Location {
        Location {
            root: root.into(),
            layout: Layout::Library,
        }
    }

    #[must_use]
    pub fn shelf(root: impl Into<PathBuf>, shelf: Shelf) -> Location {
        Location {
            root: root.into(),
            layout: Layout::Shelf(shelf),
        }
    }

    #[must_use]
    pub fn contains(&self, path: &Path) -> bool {
        path.starts_with(&self.root)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Locations {
    home: Location,
    extra: Vec<Location>,
}

impl Locations {
    pub fn standard() -> Result<Locations> {
        Ok(Locations::new(home::root()?))
    }

    #[must_use]
    pub fn new(home: impl Into<PathBuf>) -> Locations {
        Locations {
            home: Location::library(home),
            extra: Vec::new(),
        }
    }

    #[must_use]
    pub fn with_extra(mut self, extra: impl IntoIterator<Item = Location>) -> Locations {
        for location in extra {
            if location != self.home && !self.extra.contains(&location) {
                self.extra.push(location);
            }
        }
        self
    }

    #[must_use]
    pub fn home(&self) -> &Location {
        &self.home
    }

    #[must_use]
    pub fn extra(&self) -> &[Location] {
        &self.extra
    }

    pub fn iter(&self) -> impl Iterator<Item = &Location> {
        std::iter::once(&self.home).chain(&self.extra)
    }

    pub fn ensure(&self) -> Result<()> {
        let root = &self.home.root;
        std::fs::create_dir_all(root).at(root)?;
        let present = shelves_present(root)?;
        for shelf in Shelf::ALL {
            if !present.contains(&shelf) {
                let folder = root.join(shelf.folder());
                std::fs::create_dir_all(&folder).at(folder)?;
            }
        }
        Ok(())
    }

    #[must_use]
    pub fn shelf_folder(&self, shelf: Shelf) -> PathBuf {
        let root = &self.home.root;
        std::fs::read_dir(root)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|child| std::fs::metadata(child.path()).is_ok_and(|metadata| metadata.is_dir()))
            .find(|child| child.file_name().to_str().and_then(Shelf::from_folder) == Some(shelf))
            .map_or_else(|| root.join(shelf.folder()), |child| child.path())
    }
}

fn shelves_present(root: &Path) -> Result<Vec<Shelf>> {
    let mut present = Vec::new();
    for child in std::fs::read_dir(root).at(root)? {
        let child = child.at(root)?;
        let is_dir = std::fs::metadata(child.path()).is_ok_and(|metadata| metadata.is_dir());
        let shelf = child.file_name().to_str().and_then(Shelf::from_folder);
        if let (true, Some(shelf)) = (is_dir, shelf) {
            present.push(shelf);
        }
    }
    Ok(present)
}
