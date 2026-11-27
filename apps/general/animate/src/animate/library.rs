use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use animate_cast::{FigureId, Subject};
use animate_library::{
    Catalog, Entry, FigureKey, Library, Location, Locations, Shelf, ThumbnailSize, Thumbnails,
};
use animate_library_browser::{Browser, Performer, Shelving, Thumbnailing};
use animate_preferences::Preferences;
use rikka_app::prelude::*;
use rikka_dialog::{Chooser, ChooserResponse};
use rikka_ui::DragOffer;

use super::Animate;
use super::booth::{BoothPainter, Previewing};
use crate::arrival::Placement;

#[derive(Debug)]
pub(crate) struct Shelves {
    library: Option<Library>,
    catalog: Arc<Catalog>,
    locations: Locations,
    folders: [PathBuf; Shelf::ALL.len()],
    browser: Browser,
    painter: Option<Arc<BoothPainter>>,
    thumbnailing: Option<Thumbnailing>,
    previewing: Option<Previewing>,
    open: bool,
    performer: Cell<Option<FigureId>>,
    key: RefCell<Option<(FigureId, Option<FigureKey>)>>,
}

impl Shelves {
    pub(super) fn open(preferences: &Preferences, sender: &Sender<Animate>) -> (Self, Vec<String>) {
        let mut problems = Vec::new();
        let browser = Browser::new(preferences.library_shelf(), preferences.thumbnail_size());
        let locations = match Locations::standard() {
            Ok(locations) => locations.with_extra(preferences.library_locations()),
            Err(error) => {
                problems.push(error.to_string());
                return (
                    Self::closed(browser, Locations::new(PathBuf::new())),
                    problems,
                );
            }
        };
        if let Err(error) = locations.ensure() {
            problems.push(error.to_string());
        }
        let delivered = sender.clone();
        let library = match Library::open(locations.clone(), move |catalog: Arc<Catalog>| {
            let _ = delivered.send(move |animate: &mut Animate| animate.on_catalog(catalog));
        }) {
            Ok(library) => Some(library),
            Err(error) => {
                problems.push(error.to_string());
                None
            }
        };
        let painter = match BoothPainter::spawn() {
            Ok(painter) => Some(Arc::new(painter)),
            Err(error) => {
                problems.push(error);
                None
            }
        };
        let thumbnailing = match (Thumbnails::standard(), &painter) {
            (Ok(thumbnails), Some(painter)) => Some(Thumbnailing {
                thumbnails: Arc::new(thumbnails),
                painter: Arc::clone(painter) as Arc<dyn animate_library_browser::Painter>,
            }),
            (Err(error), _) => {
                problems.push(error.to_string());
                None
            }
            (Ok(_), None) => None,
        };
        let shelves = Self {
            library,
            painter,
            thumbnailing,
            ..Self::closed(browser, locations)
        };
        (shelves, problems)
    }

    fn closed(browser: Browser, locations: Locations) -> Self {
        Self {
            library: None,
            catalog: Arc::default(),
            folders: Shelf::ALL.map(|shelf| locations.shelf_folder(shelf)),
            locations,
            browser,
            painter: None,
            thumbnailing: None,
            previewing: None,
            open: true,
            performer: Cell::new(None),
            key: RefCell::new(None),
        }
    }
}

impl Animate {
    pub(crate) fn is_library_open(&self) -> bool {
        self.shelves.open
    }

    pub(crate) fn toggle_library(&mut self) {
        self.shelves.open = !self.shelves.open;
        if !self.shelves.open {
            self.end_preview();
        }
    }

    pub(crate) fn on_catalog(&mut self, catalog: Arc<Catalog>) {
        if let Some(previewed) = self.shelves.browser.previewed()
            && catalog.entry(previewed).is_none()
        {
            self.end_preview();
        }
        self.shelves.catalog = catalog;
        if !self.shelves.open {
            skip_view();
        }
    }

    pub(crate) fn dropped_entry(&mut self, offer: &DragOffer) -> bool {
        let Some(entry) = self.shelves.browser.take_lifted() else {
            return false;
        };
        let uri = rikka_open::path_to_uri(entry.path());
        if !offer.uris().contains(&uri) {
            return false;
        }
        self.take(entry);
        true
    }

    fn end_preview(&mut self) {
        self.shelves.previewing = None;
        self.shelves.browser.end_preview();
    }

    fn performing(&self) -> Option<FigureId> {
        let chosen = match self.subject {
            Some(Subject::Figure(id)) => Some(id),
            _ => self
                .shelves
                .performer
                .get()
                .filter(|id| self.cast.figure(*id).is_some()),
        };
        self.shelves.performer.set(chosen);
        chosen
    }

    fn figure_key(&self, id: FigureId) -> Option<FigureKey> {
        if let Some((known, key)) = *self.shelves.key.borrow()
            && known == id
        {
            return key;
        }
        let figure = self.cast.figure(id)?;
        let key = figure
            .cloth
            .path()
            .and_then(|path| FigureKey::from_file(path).ok());
        if let (Some(key), Some(painter)) = (key, &self.shelves.painter) {
            painter.perform(key, figure);
        }
        *self.shelves.key.borrow_mut() = Some((id, key));
        key
    }

    fn aim_at(&mut self, subject: Subject) {
        if self.subject == Some(subject) {
            return;
        }
        if let Some(index) = self
            .cast
            .subjects()
            .iter()
            .position(|candidate| *candidate == subject)
        {
            self.choose_subject(index);
        }
    }

    fn perform(&mut self, path: PathBuf) {
        let target = match self.subject {
            Some(subject @ (Subject::Figure(_) | Subject::Stage | Subject::Accessory(_))) => {
                Some(subject)
            }
            _ => self
                .performing()
                .or_else(|| self.cast.figures().first().map(|figure| figure.id))
                .map(Subject::Figure),
        };
        match target {
            Some(subject) => {
                self.aim_at(subject);
                self.load(vec![path]);
            }
            None => self.notice = Some(tr!("animate.library.no_figure")),
        }
    }
}

impl Shelving for Animate {
    fn browser(&self) -> &Browser {
        &self.shelves.browser
    }

    fn browser_mut(&mut self) -> &mut Browser {
        &mut self.shelves.browser
    }

    fn catalog(&self) -> &Arc<Catalog> {
        &self.shelves.catalog
    }

    fn locations(&self) -> &Locations {
        &self.shelves.locations
    }

    fn shelf_folder(&self, shelf: Shelf) -> &Path {
        &self.shelves.folders[shelf.index()]
    }

    fn performer(&self) -> Option<Performer> {
        let id = self.performing()?;
        let key = self.figure_key(id)?;
        let name = self.cast.figure(id)?.name.clone();
        Some(Performer { key, name })
    }

    fn thumbnailing(&self) -> Option<Thumbnailing> {
        self.shelves.thumbnailing.clone()
    }

    fn choose_shelf(&mut self, shelf: Shelf) {
        self.end_preview();
        self.shelves.browser.choose_shelf(shelf);
        self.preferences.keep_library_shelf(shelf);
    }

    fn set_thumbnail_size(&mut self, size: ThumbnailSize) {
        self.shelves.browser.set_size(size);
        self.preferences.keep_thumbnail_size(size);
    }

    fn take(&mut self, entry: Arc<Entry>) {
        if !entry.is_usable() {
            skip_view();
            return;
        }
        let path = entry.path().to_path_buf();
        match entry.shelf() {
            Shelf::Models => self.load_as(vec![path], Placement::Figure),
            Shelf::Motions => self.perform(path),
            Shelf::Cameras => {
                self.aim_at(Subject::Camera);
                self.load(vec![path]);
            }
            Shelf::Stages => self.place_stage(path),
            Shelf::Accessories => self.add_accessory(path),
            Shelf::Music => self.load(vec![path]),
        }
    }

    fn preview(&mut self, entry: Option<Arc<Entry>>) {
        self.end_preview();
        let (Some(entry), Some(painter)) = (entry, self.shelves.painter.clone()) else {
            return;
        };
        let variant = animate_library_browser::variant(&entry, self.performer().as_ref());
        if let Some(previewing) = painter.preview(&entry, variant) {
            self.shelves
                .browser
                .show_preview(entry.id(), previewing.feed());
            self.shelves.previewing = Some(previewing);
        }
    }

    fn reveal(&mut self, path: &Path) {
        skip_view();
        let path = path.to_path_buf();
        spawn_blocking(
            move || rikka_open::reveal(&path).map_err(|error| error.to_string()),
            |animate: &mut Animate, revealed: Result<(), String>| match revealed {
                Ok(()) => skip_view(),
                Err(error) => animate.notice = Some(Label::from(error)),
            },
        );
    }

    fn add_location(&mut self, shelf: Shelf) {
        let chooser = Chooser::open(tr!("animate.library.choose_folder").to_string())
            .parent(window_identifier())
            .directory(true);
        spawn_blocking(
            move || chooser.run(),
            move |animate: &mut Animate, chosen: rikka_dialog::Result<ChooserResponse>| match chosen
            {
                Ok(ChooserResponse::Files(paths)) => {
                    let added = paths.into_iter().map(|path| Location::shelf(path, shelf));
                    let extra: Vec<Location> = animate
                        .shelves
                        .locations
                        .extra()
                        .iter()
                        .cloned()
                        .chain(added)
                        .collect();
                    animate.relocate(extra);
                }
                Ok(ChooserResponse::Cancelled) => skip_view(),
                Err(error) => animate.notice = Some(Label::from(error.to_string())),
            },
        );
    }

    fn remove_location(&mut self, location: Location) {
        let extra: Vec<Location> = self
            .shelves
            .locations
            .extra()
            .iter()
            .filter(|kept| **kept != location)
            .cloned()
            .collect();
        self.relocate(extra);
    }

    fn rescan(&mut self) {
        if let Some(library) = &self.shelves.library {
            library.rescan();
        }
    }
}

impl Animate {
    fn relocate(&mut self, extra: Vec<Location>) {
        let home = self.shelves.locations.home().root.clone();
        let locations = Locations::new(home).with_extra(extra);
        self.preferences.keep_library_locations(locations.extra());
        if let Some(library) = &self.shelves.library {
            library.set_locations(locations.clone());
        }
        self.shelves.locations = locations;
    }
}
