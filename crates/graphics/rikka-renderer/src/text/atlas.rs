use std::sync::Arc;

use ash::vk;
use etagere::{AllocId, AtlasAllocator, size2};
use rikka_sync::Mutex;
use rustc_hash::FxHashSet;

use crate::text::curve_store::{CurveSlot, CurveStore, EncodedCurves};
use crate::text::pending_uploads::PendingUploads;
use crate::text::residency::Residency;
use crate::text::routing::GlyphRoute;
use rikka_gpu::core::buffer::BufferBinding;
use rikka_gpu::core::device::RenderDevice;
use rikka_gpu::core::frame_ctx::GpuFrameCtx;
use rikka_gpu::core::image::{GpuImage, ImageAccess, color_subresource_layers, record_barriers};
use rikka_gpu::core::resource_table::TextureSlots;
use rikka_gpu::core::table_image::TableImage;
use rikka_gpu::error::{Error, Result, Store};
use rikka_text::{CurveBlob, CurveKey, GlyphImage, GlyphKey, GlyphKind, GlyphRaster, IconKey};

const INITIAL_PAGE_SIZE: u32 = 512;

const SPARE_FRAMES: u64 = 120;

const GUTTER: u32 = 1;

pub(crate) type SharedAtlas = Arc<Mutex<GlyphAtlas>>;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum AtlasKey {
    Text(GlyphKey),
    Icon(IconKey),
    Curves(CurveKey),
}

fn lane(kind: GlyphKind) -> usize {
    match kind {
        GlyphKind::Mask => 0,
        GlyphKind::Color => 1,
    }
}

fn page_limits(kind: GlyphKind) -> (u32, usize) {
    match kind {
        GlyphKind::Mask => (4096, 4),
        GlyphKind::Color => (2048, 3),
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum GlyphSlot {
    Image(ImageSlot),
    Curves(CurveSlot),
}

impl GlyphSlot {
    fn drawn_at(self, scale: f32) -> Self {
        match self {
            GlyphSlot::Image(image) => GlyphSlot::Image(ImageSlot { scale, ..image }),
            curves => curves,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct GlyphRef {
    pub key: AtlasKey,
    pub slot: Option<GlyphSlot>,
}

impl GlyphRef {
    pub(crate) fn placed_key(&self) -> Option<AtlasKey> {
        self.slot.is_some().then_some(self.key)
    }
}
#[derive(Clone, Copy, Debug)]
pub(crate) struct ImageSlot {
    pub kind: GlyphKind,
    pub tex: u32,
    pub uv_min: [f32; 2],
    pub uv_max: [f32; 2],
    pub left: i32,
    pub top: i32,
    pub width: u32,
    pub height: u32,
    pub scale: f32,
}

struct PlacedGlyph {
    alloc: AllocId,
    kind: GlyphKind,
    page: u32,
    tex: u32,
    page_size: u32,
    rect_min: [u32; 2],
    size: [u32; 2],
    left: i32,
    top: i32,
}

impl PlacedGlyph {
    fn slot(&self) -> ImageSlot {
        let texel = 1.0 / self.page_size as f32;
        ImageSlot {
            kind: self.kind,
            tex: self.tex,
            uv_min: [
                self.rect_min[0] as f32 * texel,
                self.rect_min[1] as f32 * texel,
            ],
            uv_max: [
                (self.rect_min[0] + self.size[0]) as f32 * texel,
                (self.rect_min[1] + self.size[1]) as f32 * texel,
            ],
            left: self.left,
            top: self.top,
            width: self.size[0],
            height: self.size[1],
            scale: 1.0,
        }
    }
}

struct Page {
    table: TableImage,
    allocator: AtlasAllocator,
    id: u32,
    live: usize,
    emptied: Option<u64>,
    bound: bool,
}

impl Page {
    fn size(&self) -> u32 {
        self.table.image.extent().width
    }

    fn holds(&self, width: u32, height: u32) -> bool {
        self.size() >= width && self.size() >= height
    }
}

struct Placement {
    alloc: AllocId,
    page: u32,
    tex: u32,
    page_size: u32,
    rect_min: [u32; 2],
}

struct PendingCopy {
    kind: GlyphKind,
    page: u32,
    origin: [u32; 2],
    extent: [u32; 2],
    bytes: Vec<u8>,
}

pub(crate) struct GlyphAtlas {
    device: Arc<RenderDevice>,
    slots: TextureSlots,
    glyphs: Residency<AtlasKey, PlacedGlyph>,
    pages: [Vec<Page>; 2],
    minted: u32,
    frame: u64,
    copies: PendingUploads<PendingCopy>,
    curves: CurveStore,
    warming: FxHashSet<AtlasKey>,
    refused: FxHashSet<AtlasKey>,
    staged: Vec<u8>,
    starts: Vec<vk::DeviceSize>,
    barriers: Vec<vk::ImageMemoryBarrier2<'static>>,
    regions: Vec<vk::BufferImageCopy>,
}

pub(crate) enum RasterizedGlyph {
    Image(AtlasKey, Option<GlyphImage>),
    Curves(CurveKey, Option<EncodedCurves>),
}

impl RasterizedGlyph {
    pub(crate) fn key(&self) -> AtlasKey {
        match self {
            RasterizedGlyph::Image(key, _) => *key,
            RasterizedGlyph::Curves(key, _) => AtlasKey::Curves(*key),
        }
    }
}

pub(crate) fn draw(
    raster: &mut GlyphRaster,
    blob: &mut CurveBlob,
    key: AtlasKey,
) -> RasterizedGlyph {
    match key {
        AtlasKey::Text(glyph) => RasterizedGlyph::Image(key, raster.raster(glyph)),
        AtlasKey::Icon(icon) => RasterizedGlyph::Image(key, raster.raster_icon(icon)),
        AtlasKey::Curves(curves) => {
            RasterizedGlyph::Curves(curves, EncodedCurves::encode(raster, blob, curves))
        }
    }
}

impl GlyphAtlas {
    pub(crate) fn new(device: Arc<RenderDevice>, slots: TextureSlots) -> Self {
        Self {
            device: device.clone(),
            slots,
            glyphs: Residency::default(),
            pages: [Vec::new(), Vec::new()],
            minted: 0,
            frame: 1,
            copies: PendingUploads::default(),
            curves: CurveStore::new(device),
            warming: FxHashSet::default(),
            refused: FxHashSet::default(),
            staged: Vec::new(),
            starts: Vec::new(),
            barriers: Vec::new(),
            regions: Vec::new(),
        }
    }

    pub(crate) fn release(&mut self, keys: &[AtlasKey]) {
        let mut freed = false;
        for key in keys {
            freed |= match key {
                AtlasKey::Curves(curves) => self.curves.release(curves),
                _ => self.glyphs.release(key),
            };
        }
        if freed {
            self.refused.clear();
        }
    }

    pub(crate) fn release_n(&mut self, key: AtlasKey, count: u32) {
        let freed = match key {
            AtlasKey::Curves(curves) => self.curves.release_n(&curves, count),
            key => self.glyphs.release_n(&key, count),
        };
        if freed {
            self.refused.clear();
        }
    }

    pub(crate) fn lacks(&self, key: &AtlasKey) -> bool {
        !self.holds(key) && !self.refused.contains(key)
    }

    pub(crate) fn begin_warming(&mut self, key: AtlasKey) -> bool {
        self.lacks(&key) && self.warming.insert(key)
    }

    pub(crate) fn cancel_warming(&mut self, keys: &[AtlasKey]) {
        for key in keys {
            self.warming.remove(key);
        }
    }

    pub(crate) fn is_wanted(&mut self, key: &AtlasKey) -> bool {
        let missing = !self.holds(key);
        if !missing {
            self.warming.remove(key);
        }
        missing
    }

    pub(crate) fn warmed(&mut self, drawn: RasterizedGlyph, completed: u64) {
        let key = drawn.key();
        self.warming.remove(&key);
        if let Err(error) = self.insert(drawn, completed) {
            tracing::debug!("no atlas room for prefetched glyph: {error}");
            self.refused.insert(key);
        }
    }

    pub(crate) fn holds(&self, key: &AtlasKey) -> bool {
        match key {
            AtlasKey::Curves(curves) => self.curves.contains(curves),
            _ => self.glyphs.contains(key),
        }
    }

    pub(crate) fn record(&mut self, ctx: &mut GpuFrameCtx<'_>) -> Result<BufferBinding> {
        for (kind, pages) in [GlyphKind::Mask, GlyphKind::Color]
            .into_iter()
            .zip(&mut self.pages)
        {
            let sampler = match kind {
                GlyphKind::Mask => ctx.samplers.nearest_clamp(),
                GlyphKind::Color => ctx.samplers.linear_clamp(),
            };
            for page in pages.iter_mut().filter(|page| !page.bound) {
                page.table.bind(ctx.resources, sampler);
                page.bound = true;
            }
        }
        self.sweep_unused(ctx);
        self.upload(ctx)?;
        let curves = self.curves.record(ctx)?;
        self.barriers.clear();
        for page in self.pages.iter_mut().flatten() {
            self.barriers
                .extend(page.table.image.transition(ImageAccess::FRAGMENT_SAMPLED));
        }
        record_barriers(ctx.device, ctx.cmd, &self.barriers);
        self.frame = self.frame.max(ctx.frame_value + 1);
        Ok(curves)
    }

    pub(crate) fn commit(&mut self) {
        self.curves.commit();
        self.copies.commit();
    }

    pub(crate) fn revert(&mut self) {
        self.curves.revert();
        self.copies.revert();
    }

    pub(crate) fn acquire_n(
        &mut self,
        route: &GlyphRoute,
        count: u32,
    ) -> Option<Option<GlyphSlot>> {
        let frame = self.frame;
        let slot = match route.key {
            AtlasKey::Curves(curves) => self
                .curves
                .acquire_n(curves, frame, count)
                .map(|slot| slot.map(GlyphSlot::Curves)),
            key => self
                .glyphs
                .acquire_n(key, frame, count)
                .map(|placed| placed.map(|placed| GlyphSlot::Image(placed.slot()))),
        };
        slot.map(|slot| slot.map(|slot| slot.drawn_at(route.scale)))
    }

    pub(crate) fn insert(&mut self, drawn: RasterizedGlyph, completed: u64) -> Result<()> {
        match drawn {
            RasterizedGlyph::Image(key, raster) => self.insert_image(key, raster, completed),
            RasterizedGlyph::Curves(key, curves) => {
                self.curves.insert(key, curves, completed, self.frame)
            }
        }
    }

    fn insert_image(
        &mut self,
        key: AtlasKey,
        raster: Option<GlyphImage>,
        completed: u64,
    ) -> Result<()> {
        if self.glyphs.contains(&key) {
            return Ok(());
        }
        let Some(raster) = raster else {
            self.glyphs.remember_empty(key, self.frame, completed);
            return Ok(());
        };
        let (width, height) = (raster.width, raster.height);
        let kind = raster.kind;
        let room = [width + 2 * GUTTER, height + 2 * GUTTER];
        let largest = self.largest_page(kind);
        if room[0] > largest || room[1] > largest {
            tracing::warn!("{width}x{height}px glyph exceeds the {largest}px atlas page, skipping");
            self.glyphs.remember_empty(key, self.frame, completed);
            return Ok(());
        }

        let placed = self.place(kind, room, completed)?;
        self.stage(kind, &placed, [width, height], &raster.pixels);
        self.glyphs.place(
            key,
            PlacedGlyph {
                alloc: placed.alloc,
                kind,
                page: placed.page,
                tex: placed.tex,
                page_size: placed.page_size,
                rect_min: [placed.rect_min[0] + GUTTER, placed.rect_min[1] + GUTTER],
                size: [width, height],
                left: raster.left,
                top: raster.top,
            },
            self.frame,
        );
        Ok(())
    }

    fn place(&mut self, kind: GlyphKind, room: [u32; 2], completed: u64) -> Result<Placement> {
        let requested = size2(room[0] as i32, room[1] as i32);
        if let Some(placement) = allocate_on_any(&mut self.pages[lane(kind)], requested) {
            return Ok(placement);
        }
        if let Some(placement) = self.evict_for(kind, room, completed) {
            return Ok(placement);
        }
        self.add_page(kind, room[0].max(room[1]))?;
        let page = self.pages[lane(kind)].last_mut().expect("page exists");
        Ok(allocate_on(page, requested)
            .expect("empty page fits an allocation no larger than itself"))
    }

    fn evict_for(&mut self, kind: GlyphKind, room: [u32; 2], completed: u64) -> Option<Placement> {
        let requested = size2(room[0] as i32, room[1] as i32);
        let pages = &mut self.pages[lane(kind)];
        let mut evictable = self.glyphs.evictable(completed);
        let mut freed = false;
        while let Some(placed) = evictable.next_where(|placed| {
            placed.kind == kind
                && pages
                    .iter()
                    .any(|page| page.id == placed.page && page.holds(room[0], room[1]))
        }) {
            freed = true;
            let page = pages
                .iter_mut()
                .find(|page| page.id == placed.page)
                .expect("evicted glyph has a page");
            page.allocator.deallocate(placed.alloc);
            page.live -= 1;
            if let Some(placement) = allocate_on(page, requested) {
                self.refused.clear();
                return Some(placement);
            }
        }
        if freed {
            self.refused.clear();
        }
        None
    }

    fn stage(&mut self, kind: GlyphKind, placed: &Placement, extent: [u32; 2], pixels: &[u8]) {
        let texel = kind.bytes_per_pixel();
        let gutter = GUTTER as usize * texel;
        let row = extent[0] as usize * texel;
        let padded_row = row + 2 * gutter;

        let mut bytes = vec![0; padded_row * GUTTER as usize];
        for line in pixels.chunks_exact(row) {
            bytes.extend(std::iter::repeat_n(0, gutter));
            bytes.extend_from_slice(line);
            bytes.extend(std::iter::repeat_n(0, gutter));
        }
        bytes.resize(bytes.len() + padded_row * GUTTER as usize, 0);

        self.copies.push(PendingCopy {
            kind,
            page: placed.page,
            origin: placed.rect_min,
            extent: [extent[0] + 2 * GUTTER, extent[1] + 2 * GUTTER],
            bytes,
        });
    }

    fn upload(&mut self, ctx: &mut GpuFrameCtx<'_>) -> Result<()> {
        if self.copies.pending().is_empty() {
            return Ok(());
        }
        self.copies
            .pending_mut()
            .sort_by_key(|copy| (lane(copy.kind), copy.page));
        let Self {
            copies,
            pages,
            staged,
            starts,
            barriers,
            regions,
            ..
        } = self;
        let pending = copies.pending();
        staged.clear();
        starts.clear();
        for copy in pending {
            let start = staged.len().next_multiple_of(4);
            staged.resize(start, 0);
            staged.extend_from_slice(&copy.bytes);
            starts.push(start as vk::DeviceSize);
        }
        let slice = ctx.staging.stage(staged)?;
        ctx.begin_pass("rikka.glyphs");
        barriers.clear();
        for (k, lane_pages) in pages.iter_mut().enumerate() {
            for page in lane_pages {
                if pending
                    .iter()
                    .any(|copy| lane(copy.kind) == k && copy.page == page.id)
                {
                    barriers.extend(page.table.image.transition(ImageAccess::TRANSFER_DST));
                }
            }
        }
        record_barriers(ctx.device, ctx.cmd, barriers);

        let mut at = 0;
        for run in pending.chunk_by(|a, b| a.kind == b.kind && a.page == b.page) {
            regions.clear();
            for copy in run {
                regions.push(
                    vk::BufferImageCopy::default()
                        .buffer_offset(slice.offset + starts[at])
                        .image_subresource(color_subresource_layers())
                        .image_offset(vk::Offset3D {
                            x: copy.origin[0] as i32,
                            y: copy.origin[1] as i32,
                            z: 0,
                        })
                        .image_extent(vk::Extent3D {
                            width: copy.extent[0],
                            height: copy.extent[1],
                            depth: 1,
                        }),
                );
                at += 1;
            }
            let Some(page) = pages[lane(run[0].kind)]
                .iter()
                .find(|page| page.id == run[0].page)
            else {
                continue;
            };
            unsafe {
                ctx.device.handle().cmd_copy_buffer_to_image(
                    ctx.cmd,
                    slice.buffer,
                    page.table.image.handle(),
                    vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                    regions,
                );
            }
        }
        ctx.end_pass();
        copies.mark_sent();
        Ok(())
    }

    fn sweep_unused(&mut self, ctx: &mut GpuFrameCtx<'_>) {
        let pages = &mut self.pages;
        let swept = self
            .glyphs
            .sweep(ctx.frame_value, ctx.completed_value, |placed| {
                release(pages, &placed);
            });
        if swept {
            self.refused.clear();
            self.drop_spare_pages(ctx);
        }
    }

    fn drop_spare_pages(&mut self, ctx: &mut GpuFrameCtx<'_>) {
        let copies = &self.copies;
        for (k, pages) in self.pages.iter_mut().enumerate() {
            let mut index = 0;
            while index < pages.len() {
                let page = &mut pages[index];
                let copied = copies
                    .unsettled()
                    .any(|copy| lane(copy.kind) == k && copy.page == page.id);
                if page.live > 0 || copied {
                    page.emptied = None;
                    index += 1;
                    continue;
                }
                let emptied = *page.emptied.get_or_insert(ctx.frame_value);
                if ctx.completed_value < emptied + SPARE_FRAMES {
                    index += 1;
                    continue;
                }
                let page = pages.remove(index);
                page.table.retire(&mut ctx.recycler());
            }
        }
    }

    fn largest_page(&self, kind: GlyphKind) -> u32 {
        page_limits(kind)
            .0
            .min(self.device.limits().max_image_dimension2_d)
    }

    fn add_page(&mut self, kind: GlyphKind, need: u32) -> Result<()> {
        let (_, most) = page_limits(kind);
        let largest = self.largest_page(kind);
        let pages = &self.pages[lane(kind)];
        if pages.len() >= most {
            return Err(Error::Full(Store::GlyphAtlas { pages: most }));
        }
        let size = pages
            .last()
            .map_or(INITIAL_PAGE_SIZE, |last| last.size() * 2)
            .max(need.next_power_of_two())
            .min(largest);
        self.minted += 1;
        let page = self.create_page(kind, size, self.minted)?;
        self.pages[lane(kind)].push(page);
        tracing::debug!("glyph atlas {kind:?} added a {size}px page");
        Ok(())
    }

    fn create_page(&self, kind: GlyphKind, size: u32, id: u32) -> Result<Page> {
        let (format, name) = match kind {
            GlyphKind::Mask => (vk::Format::R8_UNORM, "rikka.atlas.mask"),
            GlyphKind::Color => (vk::Format::R8G8B8A8_UNORM, "rikka.atlas.color"),
        };
        let image = GpuImage::new(
            self.device.clone(),
            name,
            vk::Extent2D {
                width: size,
                height: size,
            },
            format,
            vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::TRANSFER_DST,
        )?;
        Ok(Page {
            table: TableImage::reserve(image, &self.slots)?,
            allocator: AtlasAllocator::new(size2(size as i32, size as i32)),
            id,
            live: 0,
            emptied: None,
            bound: false,
        })
    }
}

fn release(pages: &mut [Vec<Page>; 2], placed: &PlacedGlyph) {
    if let Some(page) = pages[lane(placed.kind)]
        .iter_mut()
        .find(|page| page.id == placed.page)
    {
        page.allocator.deallocate(placed.alloc);
        page.live -= 1;
    }
}

fn allocate_on(page: &mut Page, requested: etagere::Size) -> Option<Placement> {
    let alloc = page.allocator.allocate(requested)?;
    page.live += 1;
    page.emptied = None;
    Some(Placement {
        alloc: alloc.id,
        page: page.id,
        tex: page.table.tex,
        page_size: page.size(),
        rect_min: [alloc.rectangle.min.x as u32, alloc.rectangle.min.y as u32],
    })
}

fn allocate_on_any(pages: &mut [Page], requested: etagere::Size) -> Option<Placement> {
    pages
        .iter_mut()
        .find_map(|page| allocate_on(page, requested))
}
