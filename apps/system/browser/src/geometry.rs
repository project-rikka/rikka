use rikka_app::WindowShape;
use rikka_app::prelude::*;

use crate::shell::{Shell, WindowContent};
use crate::{little, topbar};
use browser_model::Geometry;

const WIDTH: f32 = 1280.0;
const HEIGHT: f32 = 820.0;
const SMALLEST: u32 = 200;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum WindowStyle {
    Main(Option<Geometry>),
    Little(f32, f32),
}

impl WindowStyle {
    pub(crate) fn for_content(content: &WindowContent) -> WindowStyle {
        match content {
            WindowContent::Little(_, little) => {
                let (width, height) = little.size();
                WindowStyle::Little(width, height)
            }
            WindowContent::Reopen(reopening, _) => WindowStyle::Main(reopening.geometry),
            _ => WindowStyle::Main(None),
        }
    }

    pub(crate) fn apply(self, window: Window<Shell>) -> Window<Shell> {
        match self {
            WindowStyle::Little(width, height) => {
                window.size(width, height).caption(little::caption_bar)
            }
            WindowStyle::Main(geometry) => {
                let (width, height) = geometry
                    .filter(|saved| saved.width >= SMALLEST && saved.height >= SMALLEST)
                    .map_or((WIDTH, HEIGHT), |saved| {
                        (saved.width as f32, saved.height as f32)
                    });
                window
                    .size(width, height)
                    .maximized(geometry.is_some_and(|saved| saved.maximized))
                    .on_shape(Shell::reshaped)
                    .caption(topbar::caption_bar)
            }
        }
    }
}

pub(crate) fn restorable(shape: &WindowShape) -> Option<Geometry> {
    let (width, height) = (shape.restored.width.round(), shape.restored.height.round());
    (width >= SMALLEST as f32 && height >= SMALLEST as f32).then_some(Geometry {
        width: width as u32,
        height: height as u32,
        maximized: shape.maximized,
    })
}

impl Shell {
    pub(crate) fn reshaped(&mut self, shape: WindowShape) {
        skip_view();
        self.shaped(&shape);
        let Some(geometry) = restorable(&shape) else {
            return;
        };
        let changed = {
            let mut model = self.browser().edit();
            match model.windows.get_mut(self.id()) {
                Some(layout) if layout.geometry != Some(geometry) => {
                    layout.geometry = Some(geometry);
                    true
                }
                _ => false,
            }
        };
        if changed {
            self.schedule_save();
        }
    }
}
