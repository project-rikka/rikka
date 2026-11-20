use std::fmt;
use std::sync::{Arc, OnceLock};

use rikka_text_catalog::FaceId;
use rikka_text_font::{Font, Location, Synthesis};
use rikka_text_shape::{Direction, SegmentProperties, ShapeFace, ShapePlan};
use rikka_text_unicode::Script;

use crate::fonts::ellipsis_glyphs::EllipsisGlyphs;
use crate::fonts::language::shaping_language;
use crate::fonts::location::LocationId;
use crate::fonts::script_equivalence::ScriptEquivalence;
use crate::style::Caps;

const SPACE: u32 = ' ' as u32;
const CAPITAL: char = 'H';
const FALLBACK_ASCENT: f32 = 0.8;
const FALLBACK_DESCENT: f32 = 0.2;
const FALLBACK_SPACE_ADVANCE: f32 = 0.25;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct EmMetrics {
    pub(crate) ascent: f32,
    pub(crate) descent: f32,
    pub(crate) underline_top: f32,
    pub(crate) underline_thickness: f32,
    pub(crate) strikeout_top: f32,
    pub(crate) strikeout_thickness: f32,
}

impl EmMetrics {
    pub(crate) const FALLBACK: Self = Self {
        ascent: FALLBACK_ASCENT,
        descent: FALLBACK_DESCENT,
        underline_top: 0.1,
        underline_thickness: 0.05,
        strikeout_top: -0.3,
        strikeout_thickness: 0.05,
    };

    fn measure(font: &Font<'_>, location: &Location) -> Self {
        let metrics = font.metrics(location);
        let scale = 1.0 / f32::from(metrics.units_per_em.max(1));
        let ascent = metrics.ascender * scale;
        let descent = -metrics.descender * scale;
        let half = (ascent + descent) / 2.0;
        let middle = metrics
            .cap_height
            .filter(|_| maps(font, CAPITAL))
            .map(|height| height * scale / 2.0)
            .filter(|&middle| middle > 0.0 && middle * 2.0 <= ascent)
            .unwrap_or((ascent - descent) / 2.0);
        Self {
            ascent: middle + half,
            descent: half - middle,
            underline_top: -metrics.underline_position * scale,
            underline_thickness: metrics.underline_thickness * scale,
            strikeout_top: -metrics.strikeout_position * scale,
            strikeout_thickness: metrics.strikeout_thickness * scale,
        }
    }
}

pub(crate) struct FaceInstance {
    pub(crate) face: FaceId,
    pub(crate) location: LocationId,
    pub(crate) synthesis: Synthesis,
    pub(crate) color: bool,
    pub(crate) font: Font<'static>,
    pub(crate) shaping: Arc<ShapeFace>,
    pub(crate) metrics: EmMetrics,
    pub(crate) units_scale: f32,
    pub(crate) space_glyph: u16,
    pub(crate) space_advance: f32,
    pub(crate) extra_advance: f32,
    pub(crate) ellipsis: OnceLock<Option<EllipsisGlyphs>>,
    equivalence: ScriptEquivalence,
}

impl FaceInstance {
    pub(crate) fn new(
        face: FaceId,
        font: Font<'static>,
        location: Location,
        location_id: LocationId,
        synthesis: Synthesis,
        color: bool,
    ) -> Self {
        let metrics = EmMetrics::measure(&font, &location);
        let units_scale = 1.0 / f32::from(font.units_per_em().max(1));
        let extra_advance = synthesis.extra_advance_em();
        let space_glyph = font
            .charmap()
            .and_then(|charmap| charmap.map(SPACE))
            .map_or(0, |glyph| glyph.0);
        let space_advance = font
            .glyph_metrics(&location)
            .advance_width(rikka_text_font::GlyphId(space_glyph))
            .map_or(FALLBACK_SPACE_ADVANCE, |advance| advance * units_scale)
            + extra_advance;
        Self {
            face,
            shaping: ShapeFace::new(font, location),
            location: location_id,
            synthesis,
            color,
            font,
            metrics,
            units_scale,
            space_glyph,
            space_advance,
            extra_advance,
            ellipsis: OnceLock::new(),
            equivalence: ScriptEquivalence::default(),
        }
    }

    pub(crate) fn plan(&self, script: Script, rtl: bool, caps: Caps) -> ShapePlan {
        let direction = if rtl {
            Direction::RightToLeft
        } else {
            Direction::LeftToRight
        };
        let properties = SegmentProperties::new(script)
            .with_direction(direction)
            .with_language(shaping_language());
        self.shaping.plan(&properties, caps.features())
    }

    pub(crate) fn shapes_equivalently(
        &self,
        first: Script,
        second: Script,
        rtl: bool,
        caps: Caps,
    ) -> bool {
        first == second
            || self
                .equivalence
                .get_or_insert((first, second, rtl, caps), || {
                    self.plan(first, rtl, caps)
                        .is_equivalent(&self.plan(second, rtl, caps))
                })
    }

    #[inline]
    pub(crate) fn maps(&self, character: char) -> bool {
        maps(&self.font, character)
    }
}

fn maps(font: &Font<'_>, character: char) -> bool {
    font.charmap()
        .and_then(|charmap| charmap.map(character as u32))
        .is_some_and(|glyph| glyph.0 != 0)
}

impl fmt::Debug for FaceInstance {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FaceInstance")
            .field("face", &self.face)
            .field("location", &self.location)
            .field("synthesis", &self.synthesis)
            .finish_non_exhaustive()
    }
}
