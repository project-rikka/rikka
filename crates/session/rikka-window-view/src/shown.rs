use std::sync::Arc;

use rikka_types::{Rect, RoundedRect, WindowView};
use rikka_wayland_client::rikka_window_view_manager_v1::RikkaWindowViewManagerV1;
use rikka_wayland_client::rikka_window_view_v1::RikkaWindowViewV1;
use rikka_wayland_client::wl_surface::WlSurface;

#[must_use]
#[derive(Debug)]
pub struct Shown {
    manager: RikkaWindowViewManagerV1,
    surface: WlSurface,
    entries: Vec<ShownEntry>,
}

#[derive(Debug)]
struct ShownEntry {
    window: Arc<str>,
    nth: usize,
    view: RikkaWindowViewV1,
    rect: Option<Rect>,
    clip: Option<Option<RoundedRect>>,
    opacity: f32,
    order: i32,
    requested: bool,
}

impl ShownEntry {
    fn send(&mut self, view: &WindowView, order: i32) {
        if self.rect != Some(view.rect) {
            self.rect = Some(view.rect);
            let rect = view.rect;
            self.view.set_destination(
                f64::from(rect.x()),
                f64::from(rect.y()),
                f64::from(rect.width()),
                f64::from(rect.height()),
            );
        }
        if self.clip != Some(view.clip) {
            self.clip = Some(view.clip);
            match view.clip {
                Some(clip) => {
                    let radii = clip.radii.clamp_for_size(clip.rect.size);
                    self.view.set_clip(
                        f64::from(clip.rect.x()),
                        f64::from(clip.rect.y()),
                        f64::from(clip.rect.width()),
                        f64::from(clip.rect.height()),
                        f64::from(radii.top_left.max(0.0)),
                        f64::from(radii.top_right.max(0.0)),
                        f64::from(radii.bottom_right.max(0.0)),
                        f64::from(radii.bottom_left.max(0.0)),
                    );
                }
                None => self.view.set_clip(0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0),
            }
        }
        let opacity = match view.opacity.is_nan() {
            true => 1.0,
            false => view.opacity.clamp(0.0, 1.0),
        };
        if self.opacity != opacity {
            self.opacity = opacity;
            self.view.set_opacity(f64::from(opacity));
        }
        if self.order != order {
            self.order = order;
            self.view.set_order(order);
        }
    }
}

impl Shown {
    pub(crate) fn new(manager: RikkaWindowViewManagerV1, surface: WlSurface) -> Shown {
        Shown {
            manager,
            surface,
            entries: Vec::new(),
        }
    }

    pub fn show(&mut self, views: &[WindowView]) {
        for entry in &mut self.entries {
            entry.requested = false;
        }
        let visible = views.iter().filter(|view| showable(view));
        for (order, view) in visible.enumerate() {
            let order = i32::try_from(order).unwrap_or(i32::MAX);
            let nth = self
                .entries
                .iter()
                .filter(|entry| entry.requested && entry.window == view.window)
                .count();
            let at = match self.entries.iter().position(|entry| {
                !entry.requested && entry.nth == nth && entry.window == view.window
            }) {
                Some(at) => at,
                None => {
                    self.entries.push(self.open(&view.window, nth));
                    self.entries.len() - 1
                }
            };
            let entry = &mut self.entries[at];
            entry.requested = true;
            entry.send(view, order);
        }
        self.entries.retain(|entry| {
            if !entry.requested {
                entry.view.destroy();
            }
            entry.requested
        });
    }

    fn open(&self, window: &Arc<str>, nth: usize) -> ShownEntry {
        ShownEntry {
            window: Arc::clone(window),
            nth,
            view: self.manager.get_window_view(&self.surface, window),
            rect: None,
            clip: None,
            opacity: 1.0,
            order: 0,
            requested: false,
        }
    }
}

impl Drop for Shown {
    fn drop(&mut self) {
        for entry in self.entries.drain(..) {
            entry.view.destroy();
        }
    }
}

fn showable(view: &WindowView) -> bool {
    let rect = view.rect;
    finite(rect)
        && rect.width() > 0.0
        && rect.height() > 0.0
        && view.clip.is_none_or(|clip| {
            let radii = clip.radii;
            finite(clip.rect)
                && clip.rect.width() >= 0.0
                && clip.rect.height() >= 0.0
                && [
                    radii.top_left,
                    radii.top_right,
                    radii.bottom_right,
                    radii.bottom_left,
                ]
                .iter()
                .all(|radius| radius.is_finite())
        })
}

fn finite(rect: Rect) -> bool {
    [rect.x(), rect.y(), rect.width(), rect.height()]
        .iter()
        .all(|value| value.is_finite())
}
