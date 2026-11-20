use std::sync::OnceLock;

use rikka_text_font::{
    Charmap, Device, F2Dot14, Font, GlyphId, Gpos, Gsub, ItemVariationStore, Location,
    OutlineScratch, RegionScalars, VariantGlyph,
};

use crate::buffer::glyph_info::GlyphInfo;
use crate::buffer::glyph_position::GlyphPosition;
use crate::face::advances::Advances;
use crate::face::cmap_cache::CmapCache;
use crate::face::glyph_classes::GlyphClasses;
use crate::face::lookup_accel::{LookupAccel, LookupAccels};
use crate::plan::feature_map::TableKind;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct GlyphExtents {
    pub(crate) x_bearing: i32,
    pub(crate) y_bearing: i32,
    pub(crate) width: i32,
    pub(crate) height: i32,
}

type LayoutVariations = Option<(ItemVariationStore<'static>, RegionScalars)>;

#[derive(Debug)]
pub(crate) struct FontContext {
    pub(crate) font: Font<'static>,
    pub(crate) location: Location,
    pub(crate) feature_variations: [Option<u32>; 2],
    pub(crate) has_kern: bool,
    pub(crate) has_cross_kern: bool,
    pub(crate) has_machine_kern: bool,
    units_per_em: u16,
    has_glyph_classes: bool,
    has_layout_variations: bool,
    charmap: Option<Charmap<'static>>,
    tables: [Option<&'static [u8]>; 2],
    lookups: [LookupAccels; 2],
    classes: OnceLock<GlyphClasses>,
    advances: Advances,
    layout_variations: OnceLock<LayoutVariations>,
    cmap_cache: CmapCache,
}

impl FontContext {
    pub(crate) fn new(font: Font<'static>, location: Location) -> Self {
        let coords = location.coords();
        let varied = !coords.is_empty();
        let gsub = font.gsub();
        let gpos = font.gpos();
        let gdef = font.gdef();
        let feature_variation = |variations: Option<rikka_text_font::FeatureVariations<'_>>| {
            variations
                .filter(|_| varied)
                .and_then(|variations| variations.find_index(coords))
        };
        let kern = font.kern();
        let has_cross_kern =
            kern.is_some_and(|kern| kern.subtables().any(|subtable| subtable.is_cross_stream()));
        let has_machine_kern = kern.is_some_and(|kern| {
            kern.is_apple() && kern.subtables().any(|subtable| subtable.format() == 1)
        });
        Self {
            feature_variations: [
                feature_variation(gsub.and_then(|gsub| gsub.feature_variations())),
                feature_variation(gpos.and_then(|gpos| gpos.feature_variations())),
            ],
            has_kern: kern.is_some(),
            has_cross_kern,
            has_machine_kern,
            units_per_em: font.units_per_em(),
            has_glyph_classes: gdef.is_some_and(|gdef| gdef.has_glyph_classes()),
            has_layout_variations: varied && gdef.is_some_and(|gdef| gdef.var_store().is_some()),
            charmap: font.charmap(),
            tables: [gsub.map(|gsub| gsub.data()), gpos.map(|gpos| gpos.data())],
            lookups: [
                LookupAccels::new(gsub.map_or(0, |gsub| gsub.lookup_count())),
                LookupAccels::new(gpos.map_or(0, |gpos| gpos.lookup_count())),
            ],
            classes: OnceLock::new(),
            advances: Advances::new(font, varied),
            layout_variations: OnceLock::new(),
            cmap_cache: CmapCache::default(),
            location,
            font,
        }
    }

    #[inline]
    pub(crate) fn coords(&self) -> &[F2Dot14] {
        self.location.coords()
    }

    #[inline]
    fn classes(&self) -> &GlyphClasses {
        self.classes
            .get_or_init(|| GlyphClasses::new(self.font.gdef().as_ref(), self.font.glyph_count()))
    }

    #[inline]
    pub(crate) fn nominal_glyph(&self, codepoint: u32) -> Option<u16> {
        if let Some(cached) = self.cmap_cache.get(codepoint) {
            return cached;
        }
        let glyph = self
            .charmap
            .as_ref()
            .and_then(|charmap| charmap.map(codepoint))
            .map(|glyph| glyph.0)
            .filter(|&glyph| glyph != 0);
        self.cmap_cache.set(codepoint, glyph);
        glyph
    }

    #[inline]
    pub(crate) fn has_glyph(&self, codepoint: u32) -> bool {
        self.nominal_glyph(codepoint).is_some()
    }

    pub(crate) fn variant_glyph(&self, codepoint: u32, selector: u32) -> Option<u16> {
        match self.charmap.as_ref()?.map_variant(codepoint, selector)? {
            VariantGlyph::UseDefault => self.nominal_glyph(codepoint),
            VariantGlyph::Glyph(glyph) => Some(glyph.0).filter(|&glyph| glyph != 0),
        }
    }

    #[inline]
    pub(crate) fn glyph_props(&self, glyph: u32) -> u16 {
        self.classes().props(glyph)
    }

    #[inline]
    pub(crate) fn has_glyph_classes(&self) -> bool {
        self.has_glyph_classes
    }

    #[inline]
    pub(crate) fn is_mark_in_set(&self, glyph: u32, set: u16) -> bool {
        self.classes().is_mark_in_set(glyph, set)
    }

    #[inline]
    pub(crate) fn advance(&self, glyph: u32) -> i32 {
        self.advances.advance(glyph, self.coords())
    }

    pub(crate) fn fill_advances(&self, infos: &[GlyphInfo], positions: &mut [GlyphPosition]) {
        self.advances.fill(infos, positions, self.coords());
    }

    #[inline]
    pub(crate) fn units_per_em(&self) -> i32 {
        i32::from(self.units_per_em)
    }

    #[inline]
    pub(crate) fn device_delta(&self, device: Option<Device<'_>>) -> i32 {
        match device {
            Some(Device::Variation(index)) => match self.layout_variations() {
                Some((store, scalars)) => store.delta_fixed(scalars, index),
                None => 0,
            },
            _ => 0,
        }
    }

    #[cold]
    fn layout_variations(&self) -> &LayoutVariations {
        self.layout_variations.get_or_init(|| {
            let store = self.font.gdef()?.var_store()?;
            let scalars = store.fixed_scalars(self.coords());
            Some((store, scalars))
        })
    }

    #[inline]
    pub(crate) fn has_layout_variations(&self) -> bool {
        self.has_layout_variations
    }

    #[inline]
    pub(crate) fn table(&self, table: TableKind) -> Option<&'static [u8]> {
        self.tables[table.index()]
    }

    pub(crate) fn gsub(&self) -> Option<Gsub<'static>> {
        self.font.gsub()
    }

    pub(crate) fn gpos(&self) -> Option<Gpos<'static>> {
        self.font.gpos()
    }

    #[inline]
    pub(crate) fn lookup(&self, table: TableKind, index: u16) -> Option<&LookupAccel> {
        self.lookups[table.index()].get(index, || match table {
            TableKind::Gsub => Some(LookupAccel::from_subst(&self.gsub()?.lookup(index)?)),
            TableKind::Gpos => Some(LookupAccel::from_pos(&self.gpos()?.lookup(index)?)),
        })
    }

    pub(crate) fn extents(&self, glyph: u32, scratch: &mut OutlineScratch) -> Option<GlyphExtents> {
        let bounds = self
            .font
            .glyph_bounds(GlyphId(glyph as u16), &self.location, scratch)?;
        if bounds.width() == 0.0 && bounds.height() == 0.0 {
            return Some(GlyphExtents::default());
        }
        let x_min = bounds.x_min.floor() as i32;
        let x_max = bounds.x_max.ceil() as i32;
        let y_min = bounds.y_min.floor() as i32;
        let y_max = bounds.y_max.ceil() as i32;
        Some(GlyphExtents {
            x_bearing: x_min,
            y_bearing: y_max,
            width: x_max - x_min,
            height: y_min - y_max,
        })
    }
}
