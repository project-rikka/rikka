use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;

use futures_util::Stream;
use rikka_icons::Icons;
use rikka_image::ColorScheme;
use rikka_ui::{IconSource, ImageSource};

use crate::footprints::Footprints;
use crate::symbols::for_categories;
use crate::{DesktopEntry, Installed};

const LOOKUP_SIZE: u32 = 48;

const SYMBOLIC: &str = "-symbolic";

#[derive(Clone, Debug)]
pub struct AppIcons(Rc<RefCell<State>>);

impl AppIcons {
    #[must_use]
    pub(crate) fn read(theme: &str, installed: Installed, scheme: ColorScheme) -> AppIcons {
        AppIcons(Rc::new(RefCell::new(State {
            icons: Icons::new(theme),
            target_theme: theme.to_owned(),
            scan: installed.scan(),
            installed,
            shipped: HashMap::new(),
            footprints: Footprints::start(scheme),
        })))
    }

    pub(crate) fn wear(&self, theme: String) {
        self.0.borrow_mut().target_theme = theme;
    }

    pub(crate) fn follow_scheme(&self, scheme: ColorScheme) {
        self.0.borrow_mut().footprints.wear(scheme);
    }

    #[must_use]
    pub fn shipped(&self, root: &Path, name: &str) -> Option<IconSource> {
        let mut state = self.0.borrow_mut();
        state.settle();
        state
            .shipped
            .entry(root.to_path_buf())
            .or_insert_with(|| Icons::with_root(root))
            .lookup(name, LOOKUP_SIZE, 1)
            .map(drawn)
    }

    #[must_use]
    pub fn named(&self, name: &str) -> Option<IconSource> {
        let mut state = self.0.borrow_mut();
        state.settle();
        state.icons.lookup(name, LOOKUP_SIZE, 1).map(drawn)
    }

    #[must_use]
    pub fn icon_for(&self, entry: Option<&DesktopEntry>) -> IconSource {
        let mut state = self.0.borrow_mut();
        state.settle();

        let themed_icon = entry
            .and_then(|entry| entry.icon.as_deref())
            .and_then(|name| state.icons.lookup(name, LOOKUP_SIZE, 1));
        match themed_icon {
            Some(path) => drawn(path),
            None => {
                let categories = entry.map_or(&[][..], |entry| entry.categories.as_slice());
                IconSource::from(for_categories(categories))
            }
        }
    }

    #[must_use]
    pub fn for_app(&self, entry: Option<&DesktopEntry>, app_id: &str) -> IconSource {
        match entry {
            Some(entry) => self.icon_for(Some(entry)),
            None => self.named(app_id).unwrap_or_else(|| self.icon_for(None)),
        }
    }

    pub fn measured(&self) -> impl Stream<Item = ()> + Send + 'static + use<> {
        self.0.borrow().footprints.changes()
    }

    #[must_use]
    pub fn footprint(&self, icon: &IconSource) -> f32 {
        match icon {
            IconSource::Picture(picture) => self.0.borrow_mut().footprints.scale(picture),
            IconSource::Glyph(_) | IconSource::Mask(_) => 1.0,
        }
    }
}

#[derive(Debug)]
pub(crate) struct State {
    icons: Icons,
    target_theme: String,
    installed: Installed,
    scan: u64,
    shipped: HashMap<PathBuf, Icons>,
    footprints: Footprints,
}

impl State {
    fn settle(&mut self) {
        if self.icons.theme() != self.target_theme {
            self.icons.set_theme(&self.target_theme);
        }
        let scan = self.installed.scan();
        if self.scan != scan {
            self.scan = scan;
            self.icons.clear_cache();
            self.shipped.clear();
            self.footprints.forget();
        }
    }
}

fn drawn(path: Arc<Path>) -> IconSource {
    let symbolic = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .is_some_and(|stem| stem.ends_with(SYMBOLIC));
    match symbolic {
        true => IconSource::Mask(ImageSource::file(path)),
        false => IconSource::from(path),
    }
}
