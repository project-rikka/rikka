use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use std::sync::Arc;

use animate_library::{
    Catalog, Entry, FigureKey, Location, Locations, Shelf, ThumbnailSize, Thumbnails, Variant,
};

use crate::browser::Browser;

pub type Desk<'a> = animate_ui::Desk<'a, dyn Shelving>;

pub type Reach = animate_ui::Reach<dyn Shelving>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Performer {
    pub key: FigureKey,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Picture {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

pub type Painting = Pin<Box<dyn Future<Output = Result<Picture, String>> + Send>>;

pub trait Painter: Send + Sync {
    fn paint(&self, entry: Arc<Entry>, variant: Variant) -> Painting;
}

#[derive(Clone)]
pub struct Thumbnailing {
    pub thumbnails: Arc<Thumbnails>,
    pub painter: Arc<dyn Painter>,
}

impl std::fmt::Debug for Thumbnailing {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Thumbnailing")
            .field("thumbnails", &self.thumbnails)
            .finish_non_exhaustive()
    }
}

pub trait Shelving {
    fn browser(&self) -> &Browser;
    fn browser_mut(&mut self) -> &mut Browser;
    fn catalog(&self) -> &Arc<Catalog>;
    fn locations(&self) -> &Locations;
    fn shelf_folder(&self, shelf: Shelf) -> &Path;
    fn performer(&self) -> Option<Performer>;
    fn thumbnailing(&self) -> Option<Thumbnailing>;
    fn choose_shelf(&mut self, shelf: Shelf);
    fn set_thumbnail_size(&mut self, size: ThumbnailSize);
    fn take(&mut self, entry: Arc<Entry>);
    fn preview(&mut self, entry: Option<Arc<Entry>>);
    fn reveal(&mut self, path: &Path);
    fn add_location(&mut self, shelf: Shelf);
    fn remove_location(&mut self, location: Location);
    fn rescan(&mut self);
}
