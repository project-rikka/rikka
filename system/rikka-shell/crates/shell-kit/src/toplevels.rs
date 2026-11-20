use rikka_seam::SeamClient;
use rikka_session::toplevels::{self, State as Windows, Toplevel};
use rikka_types::{Point, Rect};
use rikka_ui::{State, stream};

use crate::Placement;

#[derive(Debug, Default, State)]
pub struct Toplevels {
    windows: Windows,
}

impl Toplevels {
    pub fn start(&mut self, seam: &SeamClient) {
        stream(
            toplevels::states(seam),
            |target: &mut Toplevels, windows: Windows| target.windows = windows,
        );
    }

    pub fn on<'a>(&'a self, place: &'a Placement) -> impl Iterator<Item = &'a Toplevel> {
        self.windows
            .toplevels
            .iter()
            .filter(|window| window.output == place.name)
    }

    #[must_use]
    pub fn covers(&self, place: &Placement, at: Point) -> bool {
        self.on(place)
            .filter(|window| window.on_screen && !window.minimized)
            .any(|window| local(window, place).contains(at))
    }

    #[must_use]
    pub fn count(&self) -> usize {
        self.windows.toplevels.len()
    }
}

#[must_use]
pub fn local(window: &Toplevel, place: &Placement) -> Rect {
    Rect::new(
        window.bounds.x as f32 - place.position.x,
        window.bounds.y as f32 - place.position.y,
        window.bounds.width as f32,
        window.bounds.height as f32,
    )
}
