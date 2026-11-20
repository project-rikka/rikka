use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};
use std::fmt;

use rikka_compositor_geometry::{Buffer, Damage, Logical, Rect};
use rikka_compositor_pixels::{area_to_rgba, damaged_rects};
use rikka_compositor_surface::{
    BufferContent, Dmabuf, Rgba, ShmBuffer, SurfaceDamage, SurfaceId, Surfaces,
};
use rikka_compositor_types::{Format, Fourcc};
use rikka_gpu::{Pixels, Rows, Ycbcr};
use rikka_renderer::{ColorId, Renderer, TexelRect, TextureId};
use rikka_types::Color;

use super::colors::SurfaceColors;
use super::holds::{GpuHolds, Pin};
use super::imports::DmabufImports;
use super::texture_refs::TextureRefs;
use crate::color::{image_color, solid_in_srgb, ycbcr};

const REMEMBERED: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Look {
    Texture(TextureId, ColorId),
    Solid(Color),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Uploaded {
    texture: TextureId,
    shape: Shape,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Shape {
    width: u32,
    height: u32,
    format: Fourcc,
    native: bool,
}

impl Shape {
    fn holding(self, texture: TextureId) -> Uploaded {
        Uploaded {
            texture,
            shape: self,
        }
    }
}

#[derive(Debug)]
struct Change {
    generation: u64,
    damage: Option<Vec<Rect<i32, Logical>>>,
}

#[derive(Debug)]
struct Entry {
    look: Look,
    uploaded: Option<Uploaded>,
    scanout: Option<Format>,
    generation: u64,
    history: VecDeque<Change>,
}

#[derive(Default)]
struct Scratch {
    rects: Vec<Rect<i32, Buffer>>,
    texels: Vec<TexelRect>,
}

#[derive(Default)]
pub(crate) struct ClientTextures {
    entries: HashMap<SurfaceId, Entry>,
    stale: BTreeSet<SurfaceId>,
    early: HashSet<SurfaceId>,
    retired: Vec<TextureId>,
    shown: HashSet<TextureId>,
    imports: DmabufImports,
    colors: SurfaceColors,
    holds: GpuHolds,
    refs: TextureRefs<TextureId>,
    damage: SurfaceDamage,
    scratch: Scratch,
}

impl ClientTextures {
    pub(crate) fn touch(&mut self, surface: SurfaceId) {
        self.stale.insert(surface);
    }

    pub(crate) fn upload_early(
        &mut self,
        surface: SurfaceId,
        surfaces: &mut Surfaces,
        renderer: &mut Renderer,
    ) {
        self.stale.insert(surface);
        let shm = matches!(surfaces.content(surface), Some(BufferContent::Shm(_)));
        if !shm || !self.early.insert(surface) {
            return;
        }
        self.refresh_stale(surface, surfaces, renderer);
    }

    pub(crate) fn forget(&mut self, surface: SurfaceId) {
        self.stale.remove(&surface);
        self.early.remove(&surface);
        self.holds.hold(surface, None);
        self.colors.forget(surface);
        if let Some(entry) = self.entries.remove(&surface) {
            self.retired
                .extend(entry.uploaded.map(|uploaded| uploaded.texture));
        }
    }

    pub(crate) fn look(&self, surface: SurfaceId) -> Option<(Look, u64)> {
        self.entries
            .get(&surface)
            .map(|entry| (entry.look, entry.generation))
    }

    pub(crate) fn damage_since(
        &self,
        surface: SurfaceId,
        seen: u64,
    ) -> Option<Vec<Rect<i32, Logical>>> {
        let entry = self.entries.get(&surface)?;
        let oldest = entry.history.front()?.generation;
        if seen + 1 < oldest {
            return None;
        }
        let mut damage = Vec::new();
        for change in &entry.history {
            if change.generation <= seen {
                continue;
            }
            damage.extend(change.damage.as_ref()?);
        }
        Some(damage)
    }

    pub(crate) fn scanout_format(&self, surface: SurfaceId) -> Option<Format> {
        self.entries.get(&surface)?.scanout
    }

    pub(crate) fn read(&mut self, surfaces: impl IntoIterator<Item = SurfaceId>, frame: u64) {
        for surface in surfaces {
            self.holds.read(surface, frame);
        }
    }

    pub(crate) fn awaited(&self) -> Option<u64> {
        self.holds.awaited()
    }

    pub(crate) fn pin(&mut self, surface: SurfaceId) -> Option<Pin> {
        self.holds.pin(surface)
    }

    pub(crate) fn unpin(&mut self, pin: Pin, surfaces: &mut Surfaces) {
        self.holds.unpin(pin, surfaces);
    }

    pub(crate) fn release(&mut self, finished: u64, surfaces: &mut Surfaces) {
        self.holds.release(finished, surfaces);
    }

    pub(crate) fn validate(&mut self, dmabuf: &Dmabuf, renderer: &mut Renderer) -> bool {
        let guessed = Ycbcr::guessed(dmabuf.size().h.max(0) as u32);
        self.imports.texture(dmabuf, guessed, renderer).is_some()
    }

    pub(crate) fn keep(&mut self, looks: impl IntoIterator<Item = Look>) {
        for look in looks {
            if let Look::Texture(texture, color) = look {
                self.refs.keep([texture]);
                self.colors.keep(color);
            }
        }
    }

    pub(crate) fn unkeep(&mut self, looks: impl IntoIterator<Item = Look>) {
        for look in looks {
            if let Look::Texture(texture, color) = look {
                let freed = self.refs.release([texture]);
                self.retired.extend(freed);
                self.colors.unkeep(color);
            }
        }
    }

    pub(crate) fn reclaim(&mut self, surfaces: &mut Surfaces, renderer: &mut Renderer) {
        self.colors.reclaim(renderer);
        self.imports.orphan(surfaces.take_destroyed_dmabufs());
        for texture in self.retired.drain(..) {
            if !self.refs.outlive(texture) {
                renderer.textures().destroy(texture);
            }
        }
        if self.imports.is_empty() {
            return;
        }
        self.shown.clear();
        self.shown
            .extend(self.entries.values().filter_map(|entry| match entry.look {
                Look::Texture(texture, _) => Some(texture),
                Look::Solid(_) => None,
            }));
        let (shown, refs) = (&self.shown, &self.refs);
        self.imports.expire(
            |texture| shown.contains(&texture) || refs.holds(texture),
            renderer,
        );
    }

    pub(crate) fn refresh(
        &mut self,
        surface: SurfaceId,
        surfaces: &mut Surfaces,
        renderer: &mut Renderer,
    ) {
        self.early.remove(&surface);
        self.refresh_stale(surface, surfaces, renderer);
    }

    fn refresh_stale(
        &mut self,
        surface: SurfaceId,
        surfaces: &mut Surfaces,
        renderer: &mut Renderer,
    ) {
        if !self.stale.remove(&surface) {
            return;
        }
        let mut damage = std::mem::take(&mut self.damage);
        surfaces.take_damage_into(surface, &mut damage);
        self.refresh_damaged(surface, &damage, surfaces, renderer);
        self.damage = damage;
    }

    fn refresh_damaged(
        &mut self,
        surface: SurfaceId,
        damage: &SurfaceDamage,
        surfaces: &mut Surfaces,
        renderer: &mut Renderer,
    ) {
        let Some(image) = surfaces.get(surface).map(image_color) else {
            return self.forget(surface);
        };
        let representation = surfaces
            .get(surface)
            .map(|entry| entry.color_representation())
            .unwrap_or_default();
        let recolored = self.colors.refresh(surface, image, renderer);
        let Some(content) = surfaces.content(surface) else {
            return self.forget(surface);
        };
        let previous = self.entries.get(&surface).and_then(|entry| entry.uploaded);
        let refreshed = if let BufferContent::Shm(shm) = content {
            upload(shm, previous, &damage.buffer, renderer, &mut self.scratch)
        } else if let BufferContent::SinglePixel(rgba) = content {
            Some(Refreshed::Solid(solid_in_srgb(solid(rgba), &image)))
        } else if let BufferContent::Dmabuf(dmabuf) = content {
            let height = dmabuf.size().h.max(0) as u32;
            self.imports
                .texture(dmabuf, ycbcr(&representation, height), renderer)
                .map(|texture| Refreshed::Imported(texture, dmabuf.format()))
        } else {
            tracing::debug!("unsupported buffer type on surface");
            None
        };
        let hold = match refreshed {
            Some(Refreshed::Imported(..)) => surfaces.hold(surface),
            _ => None,
        };
        self.holds.hold(surface, hold);
        surfaces.uploaded(surface);
        let Some(refreshed) = refreshed else {
            return;
        };
        let surface_damage: Vec<Rect<i32, Logical>> = damage.surface.iter().collect();
        self.apply_refresh(surface, refreshed, surface_damage, recolored);
    }

    fn apply_refresh(
        &mut self,
        surface: SurfaceId,
        refreshed: Refreshed,
        damage: Vec<Rect<i32, Logical>>,
        recolored: bool,
    ) {
        let color = self.colors.color(surface);
        let damage = match recolored {
            true => None,
            false => Some(damage),
        };
        let (look, uploaded, scanout, damage) = match refreshed {
            Refreshed::Unchanged => {
                let Some(entry) = self.entries.get(&surface).filter(|_| recolored) else {
                    return;
                };
                let look = match entry.look {
                    Look::Texture(texture, _) => Look::Texture(texture, color),
                    solid => solid,
                };
                (look, entry.uploaded, entry.scanout, None)
            }
            Refreshed::Patched(uploaded) => (
                Look::Texture(uploaded.texture, color),
                Some(uploaded),
                None,
                damage,
            ),
            Refreshed::Replaced(uploaded) => (
                Look::Texture(uploaded.texture, color),
                Some(uploaded),
                None,
                None,
            ),
            Refreshed::Imported(texture, format) => {
                (Look::Texture(texture, color), None, Some(format), damage)
            }
            Refreshed::Solid(color) => (Look::Solid(color), None, None, None),
        };
        let entry = self.entries.entry(surface).or_insert(Entry {
            look,
            uploaded: None,
            scanout: None,
            generation: 0,
            history: VecDeque::new(),
        });
        if let Some(old) = entry.uploaded
            && Some(old) != uploaded
        {
            self.retired.push(old.texture);
        }
        entry.look = look;
        entry.uploaded = uploaded;
        entry.scanout = scanout;
        entry.generation += 1;
        entry.history.push_back(Change {
            generation: entry.generation,
            damage,
        });
        while entry.history.len() > REMEMBERED {
            entry.history.pop_front();
        }
    }
}

enum Refreshed {
    Imported(TextureId, Format),
    Unchanged,
    Patched(Uploaded),
    Replaced(Uploaded),
    Solid(Color),
}

fn upload(
    shm: ShmBuffer<'_>,
    previous: Option<Uploaded>,
    damage: &Damage<Buffer>,
    renderer: &mut Renderer,
    scratch: &mut Scratch,
) -> Option<Refreshed> {
    let size = shm.size();
    let (width, height) = (size.w.max(0) as u32, size.h.max(0) as u32);
    let format = shm.format();
    let reused = previous.filter(|uploaded| {
        let shape = uploaded.shape;
        shape.width == width && shape.height == height && shape.format == format
    });
    match reused {
        Some(_) => damaged_rects(damage.iter(), size, &mut scratch.rects),
        None => {
            scratch.rects.clear();
            scratch.rects.push(Rect::from_size(size));
        }
    }
    if scratch.rects.is_empty() {
        return Some(Refreshed::Unchanged);
    }
    let native = match reused {
        Some(uploaded) => uploaded.shape.native,
        None => AsRef::<rikka_renderer::Uploads>::as_ref(renderer).supports_drm_format(format.0),
    };
    let shape = Shape {
        width,
        height,
        format,
        native,
    };
    match native {
        true => upload_rows(shm, reused, shape, renderer, scratch),
        false => upload_converted(shm, reused, shape, renderer, &scratch.rects),
    }
}

fn upload_rows(
    shm: ShmBuffer<'_>,
    reused: Option<Uploaded>,
    made: Shape,
    renderer: &mut Renderer,
    scratch: &mut Scratch,
) -> Option<Refreshed> {
    let textures = renderer.textures();
    let uploaded = match reused {
        Some(uploaded) => uploaded,
        None => match textures.create_drm(made.format.0, made.width, made.height) {
            Ok(texture) => made.holding(texture),
            Err(error) => {
                tracing::warn!("cannot upload surface: {error}");
                return None;
            }
        },
    };
    scratch.texels.clear();
    scratch.texels.extend(scratch.rects.iter().map(|rect| {
        TexelRect::new(
            [rect.loc.x as u32, rect.loc.y as u32],
            [rect.size.w as u32, rect.size.h as u32],
        )
    }));
    let staged = shm.with_pixels(|pixels| {
        textures.stage(
            uploaded.texture,
            Rows::new(pixels, shm.stride()),
            &scratch.texels,
        )
    });
    let sent = match staged {
        Ok(staged) => staged.and_then(|staged| staged.send()),
        Err(error) => Err(rikka_renderer::Error::Invalid(error.to_string())),
    };
    match (sent, reused) {
        (Ok(()), Some(_)) => Some(Refreshed::Patched(uploaded)),
        (Ok(()), None) => Some(Refreshed::Replaced(uploaded)),
        (Err(error), reused) => {
            tracing::warn!("cannot update surface texture: {error}");
            if reused.is_none() {
                textures.destroy(uploaded.texture);
            }
            None
        }
    }
}

fn upload_converted(
    shm: ShmBuffer<'_>,
    reused: Option<Uploaded>,
    made: Shape,
    renderer: &mut Renderer,
    rects: &[Rect<i32, Buffer>],
) -> Option<Refreshed> {
    let area = rects
        .iter()
        .copied()
        .reduce(|area, rect| area.merge(rect))?;
    let format = made.format;
    let converted = shm
        .with_pixels(|pixels| area_to_rgba(pixels, format, shm.stride(), area))
        .ok()?;
    let Some(pixels) = converted else {
        tracing::warn!(
            "cannot read {format:?} buffer of {}x{}",
            made.width,
            made.height
        );
        return None;
    };
    let textures = renderer.textures();
    match reused {
        Some(uploaded) => match textures.update_region(
            uploaded.texture,
            [area.loc.x as u32, area.loc.y as u32],
            Pixels::premultiplied(area.size.w as u32, area.size.h as u32, pixels),
        ) {
            Ok(()) => Some(Refreshed::Patched(uploaded)),
            Err(error) => {
                tracing::warn!("cannot update surface texture: {error}");
                None
            }
        },
        None => match textures.create(Pixels::premultiplied(made.width, made.height, pixels)) {
            Ok(texture) => Some(Refreshed::Replaced(made.holding(texture))),
            Err(error) => {
                tracing::warn!("cannot upload surface: {error}");
                None
            }
        },
    }
}

fn solid(rgba: Rgba) -> Color {
    let channel = |value: u32| f64::from(value) / f64::from(u32::MAX);
    let alpha = channel(rgba.a);
    if alpha <= f64::EPSILON {
        return Color::TRANSPARENT;
    }
    let straight = |value: u32| (channel(value) / alpha).min(1.0) as f32;
    Color::rgba(
        straight(rgba.r),
        straight(rgba.g),
        straight(rgba.b),
        alpha as f32,
    )
}

impl fmt::Debug for ClientTextures {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ClientTextures")
            .field("surfaces", &self.entries.len())
            .field("stale", &self.stale.len())
            .field("retired", &self.retired.len())
            .finish()
    }
}
