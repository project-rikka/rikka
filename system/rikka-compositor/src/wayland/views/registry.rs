use std::collections::HashSet;

use rikka_compositor_desktop::WindowId;
use rikka_compositor_geometry::{Logical, Rect};
use rikka_compositor_surface::{Cached, SurfaceId, Surfaces};
use rikka_types::RoundedRect;
use slotmap::{SlotMap, new_key_type};

use crate::state::Compositor;

new_key_type! {
    pub(crate) struct ViewId;
}

rikka_compositor_server::slot_keys!(ViewId);

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Look {
    pub(crate) destination: Option<Rect<f64, Logical>>,
    pub(crate) clip: Option<RoundedRect>,
    pub(crate) opacity: f32,
    pub(crate) order: i32,
}

impl Default for Look {
    fn default() -> Self {
        Self {
            destination: None,
            clip: None,
            opacity: 1.0,
            order: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Portal {
    pub(crate) view: ViewId,
    pub(crate) window: Option<WindowId>,
    pub(crate) look: Look,
}

#[derive(Debug)]
struct View {
    surface: SurfaceId,
    window: Option<WindowId>,
    look: Look,
    born: u64,
}

#[derive(Debug, Default)]
pub(crate) struct Views {
    views: SlotMap<ViewId, View>,
    stale: HashSet<SurfaceId>,
    born: u64,
}

impl Views {
    pub(super) fn open(&mut self, surface: SurfaceId, window: Option<WindowId>) -> ViewId {
        self.born += 1;
        self.views.insert(View {
            surface,
            window,
            look: Look::default(),
            born: self.born,
        })
    }

    pub(super) fn change(&mut self, view: ViewId, change: impl FnOnce(&mut Look)) {
        if let Some(entry) = self.views.get_mut(view) {
            change(&mut entry.look);
            self.stale.insert(entry.surface);
        }
    }

    pub(super) fn close(&mut self, view: ViewId) {
        if let Some(entry) = self.views.remove(view) {
            self.stale.insert(entry.surface);
        }
    }

    pub(crate) fn forget(&mut self, surface: SurfaceId) {
        self.stale.remove(&surface);
    }

    pub(crate) fn take_staged(&mut self, surface: SurfaceId) -> Option<Portals> {
        if !self.stale.remove(&surface) {
            return None;
        }
        let mut mine: Vec<(ViewId, &View)> = self
            .views
            .iter()
            .filter(|(_, entry)| entry.surface == surface)
            .collect();
        mine.sort_by_key(|(_, entry)| (entry.look.order, entry.born));
        Some(Portals(
            mine.into_iter()
                .map(|(view, entry)| Portal {
                    view,
                    window: entry.window,
                    look: entry.look,
                })
                .collect(),
        ))
    }
}

#[derive(Debug, Default)]
pub(crate) struct Portals(Vec<Portal>);

impl Cached<Compositor> for Portals {}

pub(crate) fn portals(surfaces: &Surfaces, surface: SurfaceId) -> &[Portal] {
    surfaces
        .cached::<Portals>(surface)
        .map_or(&[], |portals| portals.0.as_slice())
}
