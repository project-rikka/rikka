use std::collections::HashMap;

use rikka_compositor_desktop::{Drawn, WindowId};
use rikka_compositor_geometry::{Logical, Point, Rect};
use rikka_compositor_surface::{SurfaceId, Surfaces};
use rikka_types::{Point as ScenePoint, RoundedRect};

use super::{ButtonLight, Windows, window_shape};
use crate::policy::CaptionLook;
use crate::render::{Moved, Portrayed, Visual, WindowShape};
use crate::wayland::{Portal, portals};

pub(crate) fn portrayed(
    windows: &Windows,
    surfaces: &Surfaces,
    drawn: &[Drawn],
    look: CaptionLook,
    rounding: impl Fn(WindowId) -> Option<f32>,
) -> HashMap<SurfaceId, Vec<Portrayed>> {
    let mut portrayals = HashMap::new();
    for item in drawn {
        let Drawn::Surface { root, location, .. } = *item else {
            continue;
        };
        let portals = portals(surfaces, root);
        if portals.is_empty() {
            continue;
        }
        let at = ScenePoint::new(location.x as f32, location.y as f32);
        let portrayed: Vec<Portrayed> = portals
            .iter()
            .filter_map(|portal| {
                view_portrayal(windows, portal, look, &rounding, location.to_f64(), at)
            })
            .collect();
        portrayals.insert(root, portrayed);
    }
    portrayals
}

fn view_portrayal(
    windows: &Windows,
    portal: &Portal,
    look: CaptionLook,
    rounding: impl Fn(WindowId) -> Option<f32>,
    origin: Point<f64, Logical>,
    at: ScenePoint,
) -> Option<Portrayed> {
    let destination = portal.look.destination?;
    if portal.look.opacity <= 0.0 {
        return None;
    }
    let window = portal.window?;
    let record = windows.desktop.window(window)?;
    let focus = match windows.recency.current() == Some(window) {
        true => 1.0,
        false => 0.0,
    };
    Some(Portrayed {
        view: portal.view,
        root: record.surface()?,
        drawn_at: record.surface_location(),
        moved: Moved {
            geometry: record.geometry(),
            shown: Visual {
                rect: Rect::new(destination.loc + origin, destination.size),
                alpha: portal.look.opacity,
            },
        },
        clip: portal
            .look
            .clip
            .map(|clip| RoundedRect::new(clip.rect.translate(at), clip.radii)),
        shape: window_shape(record).map(|shape| WindowShape {
            rounding: rounding(window).unwrap_or(shape.rounding),
            ..shape
        }),
        caption: windows.caption_paint(window, look, focus, |_| ButtonLight::default()),
    })
}
