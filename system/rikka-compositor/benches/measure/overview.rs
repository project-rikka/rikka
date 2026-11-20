use std::f32::consts::TAU;
use std::sync::Arc;

use rikka_seam::Request;
use rikka_session::surfaces::OVERVIEW;
use rikka_session::toplevels::{CAPABILITY, STATE, State};
use rikka_types::{Rect, RoundedRect, WindowView};
use rikka_wayland::Host;
use rikka_wayland_client::ext_background_effect_surface_v1::ExtBackgroundEffectSurfaceV1;
use rikka_wayland_client::{zwlr_layer_shell_v1, zwlr_layer_surface_v1};
use rikka_window_view::{Shown, WindowViews};

use super::client::Painted;
use super::desk::{Desk, paint_window};

const STAGE: u32 = 40;
const EXTRA: [u32; 4] = [30, 31, 32, 33];
const SWING_FRAMES: f32 = 60.0;
const TINT: [u8; 4] = [0, 0, 0, 107];
const THUMB_SCALE: f32 = 0.12;
const THUMB_RADIUS: f32 = 8.0;
const SLOT_FILL: f32 = 0.88;

struct Seen {
    id: Arc<str>,
    real: Rect,
    slot: Rect,
}

pub(crate) struct Overview {
    stage: Painted,
    _frost: ExtBackgroundEffectSurfaceV1,
    extra: Vec<Painted>,
    shown: Shown,
    _views: WindowViews,
    _host: Host,
    windows: Vec<Seen>,
    thumb: Rect,
}

impl Overview {
    pub(crate) fn open(desk: &mut Desk) -> Overview {
        let extent = desk.extent;
        let size = (
            (extent.width as i32 * 30 / 100).max(16),
            (extent.height as i32 * 36 / 100).max(16),
        );
        let extra: Vec<Painted> = EXTRA
            .iter()
            .map(|&tag| {
                let window = desk.client.window(tag, size);
                paint_window(&window, tag as i32 * 5);
                desk.client
                    .frost(&window.surface, &[(0, 0, size.0, size.1)]);
                desk.client.show(&window, &[(0, 0, size.0, size.1)]);
                desk.client.await_frame(tag, 0);
                window
            })
            .collect();

        let stage = desk.client.layer(
            STAGE,
            zwlr_layer_shell_v1::Layer::Overlay,
            zwlr_layer_surface_v1::Anchor::all(),
            (0, 0),
            OVERVIEW,
        );
        let whole = (0, 0, stage.canvas.width, stage.canvas.height);
        stage
            .canvas
            .paint(0..stage.canvas.height, 0..stage.canvas.width, |_, _| TINT);
        let frost = desk.client.frost(&stage.surface, &[whole]);
        let before = desk.client.frames(STAGE);
        desk.client.show(&stage, &[whole]);
        desk.client.await_frame(STAGE, before);

        let screen = Rect::new(0.0, 0.0, extent.width as f32, extent.height as f32);
        let state: State = desk.session.ask(Request::new(CAPABILITY, STATE));
        let reals: Vec<(Arc<str>, Rect)> = state
            .toplevels
            .iter()
            .map(|toplevel| {
                let bounds = toplevel.bounds;
                let real = Rect::new(
                    bounds.x as f32,
                    bounds.y as f32,
                    bounds.width as f32,
                    bounds.height as f32,
                );
                (Arc::from(toplevel.id.as_str()), real)
            })
            .collect();
        let slots = slots(screen, reals.iter().map(|(_, real)| *real));
        let windows = reals
            .into_iter()
            .zip(slots)
            .map(|((id, real), slot)| Seen { id, real, slot })
            .collect();

        let host = Host::read_by_owner(desk.client.display());
        let views = WindowViews::bind(&host).expect("window view global");
        let shown = views.inside(&stage.surface);
        let thumb_size = (screen.width() * THUMB_SCALE, screen.height() * THUMB_SCALE);
        Overview {
            stage,
            _frost: frost,
            extra,
            shown,
            _views: views,
            _host: host,
            windows,
            thumb: Rect::new(
                (screen.width() - thumb_size.0) / 2.0,
                screen.height() * 0.04,
                thumb_size.0,
                thumb_size.1,
            ),
        }
    }

    pub(crate) fn swing(&mut self, desk: &mut Desk, frames: usize) {
        let mut views = Vec::with_capacity(self.windows.len() * 2);
        for frame in 0..frames {
            let spread = 0.5 - 0.5 * (TAU * frame as f32 / SWING_FRAMES).cos();
            views.clear();
            let thumb_clip = RoundedRect::new(self.thumb, THUMB_RADIUS);
            views.extend(self.windows.iter().map(|seen| {
                let placed = Rect::new(
                    self.thumb.x() + seen.real.x() * THUMB_SCALE,
                    self.thumb.y() + seen.real.y() * THUMB_SCALE,
                    seen.real.width() * THUMB_SCALE,
                    seen.real.height() * THUMB_SCALE,
                );
                WindowView::new(Arc::clone(&seen.id), placed)
                    .clip(thumb_clip)
                    .opacity(spread)
            }));
            views.extend(self.windows.iter().map(|seen| {
                WindowView::new(Arc::clone(&seen.id), lerp(seen.real, seen.slot, spread))
            }));
            self.shown.show(&views);
            let before = desk.client.frames(STAGE);
            desk.client.tick(&self.stage.surface, STAGE);
            desk.client.await_frame(STAGE, before);
        }
    }

    pub(crate) fn close(mut self, desk: &mut Desk) {
        self.shown.show(&[]);
        self.stage.surface.attach(None, 0, 0);
        self.stage.surface.commit();
        for window in self.extra.drain(..) {
            desk.client.destroy(window.tag, &window.surface);
        }
        desk.client.roundtrip();
    }
}

fn slots(screen: Rect, reals: impl ExactSizeIterator<Item = Rect>) -> Vec<Rect> {
    let count = reals.len().max(1);
    let columns = (count as f32).sqrt().ceil() as usize;
    let rows = count.div_ceil(columns);
    let area = Rect::new(
        screen.width() * 0.08,
        screen.height() * 0.22,
        screen.width() * 0.84,
        screen.height() * 0.70,
    );
    let cell = (area.width() / columns as f32, area.height() / rows as f32);
    reals
        .enumerate()
        .map(|(index, real)| {
            let (column, row) = (index % columns, index / columns);
            let fit =
                (cell.0 / real.width().max(1.0)).min(cell.1 / real.height().max(1.0)) * SLOT_FILL;
            let (width, height) = (real.width() * fit, real.height() * fit);
            Rect::new(
                area.x() + cell.0 * (column as f32 + 0.5) - width / 2.0,
                area.y() + cell.1 * (row as f32 + 0.5) - height / 2.0,
                width,
                height,
            )
        })
        .collect()
}

fn lerp(from: Rect, to: Rect, progress: f32) -> Rect {
    let mix = |a: f32, b: f32| a + (b - a) * progress;
    Rect::new(
        mix(from.x(), to.x()),
        mix(from.y(), to.y()),
        mix(from.width(), to.width()),
        mix(from.height(), to.height()),
    )
}
