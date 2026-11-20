use std::sync::atomic::{AtomicU64, Ordering};

use rikka_compositor_surface::{
    BufferContent, BufferHold, BufferId, SurfaceDamage, SurfaceId, Surfaces,
};
use rikka_picture::{Feed, ImportedBuffer, ImportedFrame, ImportedPlane, Lease, Pulse};
use rikka_types::{Color, Rect};
use slotmap::{SecondaryMap, SlotMap};

use super::pixels;
use super::state::State;
use crate::command::Command;
use crate::frame::{LayerContent, LayerId};

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

slotmap::new_key_type! {
    pub(crate) struct HoldId;
}

struct Entry {
    layer: LayerId,
    imported: ImportedBuffer,
    feed: Feed,
    shown: Option<LayerContent>,
    dirty: bool,
}

impl Entry {
    fn new() -> Self {
        Self {
            layer: LayerId(next_id()),
            imported: ImportedBuffer::new(),
            feed: Feed::new(),
            shown: None,
            dirty: true,
        }
    }
}

#[derive(Default)]
pub(crate) struct Contents {
    entries: SecondaryMap<SurfaceId, Entry>,
    holds: SlotMap<HoldId, BufferHold>,
    buffers: SecondaryMap<BufferId, u64>,
    damage: SurfaceDamage,
}

enum BufferSnapshot {
    Solid(Color),
    Pixels {
        width: u32,
        height: u32,
        rgba: Vec<u8>,
    },
    Dmabuf {
        buffer: BufferId,
        width: u32,
        height: u32,
        fourcc: u32,
        modifier: u64,
        planes: Vec<ImportedPlane>,
    },
}

impl Contents {
    pub(crate) fn layer_id(&mut self, surface: SurfaceId) -> LayerId {
        self.entry(surface).layer
    }

    pub(crate) fn touch(&mut self, surface: SurfaceId) {
        self.entry(surface).dirty = true;
    }

    pub(crate) fn forget(&mut self, surface: SurfaceId) {
        self.entries.remove(surface);
    }

    pub(crate) fn release(&mut self, surfaces: &mut Surfaces, hold: HoldId) {
        if let Some(hold) = self.holds.remove(hold) {
            surfaces.release(hold);
        }
    }

    fn entry(&mut self, surface: SurfaceId) -> &mut Entry {
        if !self.entries.contains_key(surface) {
            self.entries.insert(surface, Entry::new());
        }
        &mut self.entries[surface]
    }

    fn identity(&mut self, buffer: BufferId) -> u64 {
        if let Some(known) = self.buffers.get(buffer) {
            return *known;
        }
        let allocated = next_id();
        self.buffers.insert(buffer, allocated);
        allocated
    }
}

impl State {
    pub(crate) fn content_of(&mut self, surface: SurfaceId) -> Option<LayerContent> {
        let entry = self.contents.entry(surface);
        if !std::mem::replace(&mut entry.dirty, false) {
            return entry.shown.clone();
        }
        let shown = self
            .fetch(surface)
            .and_then(|snapshot| self.show(surface, snapshot));
        self.contents.entry(surface).shown.clone_from(&shown);
        shown
    }

    fn fetch(&mut self, surface: SurfaceId) -> Option<BufferSnapshot> {
        let buffer = self.surfaces.get(surface)?.buffer()?;
        Some(match self.surfaces.content(surface)? {
            BufferContent::SinglePixel(rgba) => BufferSnapshot::Solid(color_of(rgba)),
            BufferContent::Shm(shm) => {
                let size = shm.size();
                let (width, height) = (size.w.max(0) as usize, size.h.max(0) as usize);
                let rgba = shm
                    .with_pixels(|bytes| {
                        pixels::rgba(shm.format(), width, height, shm.stride(), bytes)
                    })
                    .ok()
                    .flatten();
                let Some(rgba) = rgba else {
                    tracing::debug!("failed to read nested shm buffer");
                    return None;
                };
                BufferSnapshot::Pixels {
                    width: width as u32,
                    height: height as u32,
                    rgba,
                }
            }
            BufferContent::Dmabuf(dmabuf) => {
                let planes = dmabuf
                    .planes()
                    .iter()
                    .map(|plane| {
                        Some(ImportedPlane {
                            fd: plane.fd().try_clone_to_owned().ok()?,
                            offset: plane.offset(),
                            stride: plane.stride(),
                        })
                    })
                    .collect::<Option<Vec<ImportedPlane>>>()?;
                let format = dmabuf.format();
                BufferSnapshot::Dmabuf {
                    buffer,
                    width: dmabuf.size().w.max(0) as u32,
                    height: dmabuf.size().h.max(0) as u32,
                    fourcc: format.fourcc.0,
                    modifier: format.modifier.0,
                    planes,
                }
            }
        })
    }

    fn show(&mut self, surface: SurfaceId, snapshot: BufferSnapshot) -> Option<LayerContent> {
        let damage = self.damage_of(surface);
        match snapshot {
            BufferSnapshot::Solid(color) => {
                self.surfaces.uploaded(surface);
                Some(LayerContent::Solid(color))
            }
            BufferSnapshot::Pixels {
                width,
                height,
                rgba,
            } => {
                self.surfaces.uploaded(surface);
                let feed = self.contents.entry(surface).feed.clone();
                feed.show(width, height, rgba);
                Some(LayerContent::Feed(feed))
            }
            BufferSnapshot::Dmabuf {
                buffer,
                width,
                height,
                fourcc,
                modifier,
                planes,
            } => {
                let damage = damage.filter(|rects| !covers(rects, width, height));
                let hold = self.surfaces.hold(surface)?;
                self.surfaces.uploaded(surface);
                let hold = self.contents.holds.insert(hold);
                let identity = self.contents.identity(buffer);
                let released = self.link.clone();
                let lease = Lease::new().on_released(move || {
                    let _ = released.send(Command::Released(hold));
                });
                let pulse = self.take_view_pulse(surface);
                let imported = self.contents.entry(surface).imported.clone();
                imported.show(ImportedFrame {
                    buffer: identity,
                    width,
                    height,
                    fourcc,
                    modifier,
                    planes,
                    lease: Some(lease),
                    damage,
                    pulse,
                });
                Some(LayerContent::Imported(imported))
            }
        }
    }

    fn take_view_pulse(&mut self, surface: SurfaceId) -> Option<Pulse> {
        let view = self.view_of(surface)?;
        let record = self.record(view)?;
        record.wake_pending = false;
        Some(record.shared.pulse().clone())
    }

    fn damage_of(&mut self, surface: SurfaceId) -> Option<Vec<Rect>> {
        self.surfaces
            .take_damage_into(surface, &mut self.contents.damage);
        let damage = &self.contents.damage.buffer;
        (!damage.is_empty()).then(|| {
            damage
                .iter()
                .map(|rect| {
                    Rect::new(
                        rect.loc.x as f32,
                        rect.loc.y as f32,
                        rect.size.w as f32,
                        rect.size.h as f32,
                    )
                })
                .collect()
        })
    }
}

fn covers(rects: &[Rect], width: u32, height: u32) -> bool {
    rects.iter().any(|rect| {
        rect.origin.x <= 0.0
            && rect.origin.y <= 0.0
            && rect.size.width >= width as f32
            && rect.size.height >= height as f32
    })
}

fn color_of(rgba: rikka_compositor_surface::Rgba) -> Color {
    let channel = |value: u32| value as f64 / u32::MAX as f64;
    let alpha = channel(rgba.a);
    let straight = |value: u32| match alpha > 0.0 {
        true => (channel(value) / alpha).min(1.0) as f32,
        false => 0.0,
    };
    Color::rgba(
        straight(rgba.r),
        straight(rgba.g),
        straight(rgba.b),
        alpha as f32,
    )
}

fn next_id() -> u64 {
    NEXT_ID.fetch_add(1, Ordering::Relaxed)
}
