use crate::outline::bounds::Bounds;
use crate::outline::scratch::OutlineScratch;
use crate::sfnt::font::Font;
use crate::tables::os2::Selection;
use crate::variations::location::Location;
use crate::variations::mvar::Mvar;

const FALLBACK_ASCENDER_EM: f32 = 0.8;
const FALLBACK_DESCENDER_EM: f32 = 0.2;
const FALLBACK_UNDERLINE_THICKNESS_EM: f32 = 1.0 / 14.0;
const FALLBACK_STRIKEOUT_ASCENDER_RATIO: f32 = 0.3;
const FALLBACK_STRIKEOUT_X_HEIGHT_RATIO: f32 = 0.5;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FontMetrics {
    pub units_per_em: u16,
    pub ascender: f32,
    pub descender: f32,
    pub line_gap: f32,
    pub x_height: Option<f32>,
    pub cap_height: Option<f32>,
    pub underline_position: f32,
    pub underline_thickness: f32,
    pub strikeout_position: f32,
    pub strikeout_thickness: f32,
    pub italic_angle: f32,
    pub is_monospace: bool,
    pub bounds: Bounds,
}

impl Font<'_> {
    pub fn metrics(&self, location: &Location) -> FontMetrics {
        let coords = location.coords();
        let mvar = if coords.is_empty() { None } else { self.mvar() };
        let delta = |tag| mvar.map_or(0.0, |mvar| mvar.delta(tag, coords));
        let os2 = self.os2();
        let hhea = self.hhea();
        let typo = os2.and_then(|os2| {
            Some((
                os2.typo_ascender()?,
                os2.typo_descender()?,
                os2.typo_line_gap()?,
            ))
        });
        let use_typo = os2.is_some_and(|os2| os2.selection().contains(Selection::USE_TYPO_METRICS));
        let hhea_values = hhea
            .map(|hhea| (hhea.ascender(), hhea.descender(), hhea.line_gap()))
            .filter(|&(ascender, descender, _)| ascender != 0 || descender != 0);
        let win = os2.and_then(|os2| Some((os2.win_ascent()?, os2.win_descent()?)));
        let units_per_em = self.units_per_em();
        let (ascender, descender, line_gap) = match (use_typo, typo, hhea_values, win) {
            (true, Some(typo), _, _) => typo_floats(typo),
            (_, _, Some(hhea), _) => typo_floats(hhea),
            (_, Some(typo), _, _) if typo.0 != 0 || typo.1 != 0 => typo_floats(typo),
            (_, _, _, Some((ascent, descent))) => (f32::from(ascent), -f32::from(descent), 0.0),
            _ => (
                f32::from(units_per_em) * FALLBACK_ASCENDER_EM,
                -f32::from(units_per_em) * FALLBACK_DESCENDER_EM,
                0.0,
            ),
        };
        let ascender = ascender + delta(Mvar::HORIZONTAL_ASCENDER);
        let descender = descender + delta(Mvar::HORIZONTAL_DESCENDER);
        let line_gap = line_gap + delta(Mvar::HORIZONTAL_LINE_GAP);
        let x_height = os2
            .and_then(|os2| os2.x_height())
            .filter(|&height| height > 0)
            .map(|height| f32::from(height) + delta(Mvar::X_HEIGHT))
            .or_else(|| self.glyph_top('x', location));
        let cap_height = os2
            .and_then(|os2| os2.cap_height())
            .filter(|&height| height > 0)
            .map(|height| f32::from(height) + delta(Mvar::CAP_HEIGHT))
            .or_else(|| self.glyph_top('H', location));
        let post = self.post();
        let underline_position = post.map_or(0.0, |post| f32::from(post.underline_position()))
            + delta(Mvar::UNDERLINE_OFFSET);
        let underline_thickness = post
            .map(|post| f32::from(post.underline_thickness()))
            .filter(|&thickness| thickness > 0.0)
            .unwrap_or(f32::from(units_per_em) * FALLBACK_UNDERLINE_THICKNESS_EM)
            + delta(Mvar::UNDERLINE_SIZE);
        let strikeout_thickness = os2
            .map(|os2| f32::from(os2.strikeout_size()))
            .filter(|&size| size > 0.0)
            .map_or(underline_thickness, |size| {
                size + delta(Mvar::STRIKEOUT_SIZE)
            });
        let strikeout_position = os2
            .map(|os2| f32::from(os2.strikeout_position()))
            .filter(|&position| position != 0.0)
            .map(|position| position + delta(Mvar::STRIKEOUT_OFFSET))
            .unwrap_or_else(|| {
                x_height.map_or(ascender * FALLBACK_STRIKEOUT_ASCENDER_RATIO, |height| {
                    height * FALLBACK_STRIKEOUT_X_HEIGHT_RATIO
                })
            });
        FontMetrics {
            units_per_em,
            ascender,
            descender,
            line_gap,
            x_height,
            cap_height,
            underline_position,
            underline_thickness,
            strikeout_position,
            strikeout_thickness,
            italic_angle: post.map_or(0.0, |post| post.italic_angle()),
            is_monospace: post.is_some_and(|post| post.is_fixed_pitch()),
            bounds: self.head().map(|head| head.bounds()).unwrap_or_default(),
        }
    }

    fn glyph_top(&self, character: char, location: &Location) -> Option<f32> {
        let glyph = self.charmap()?.map(u32::from(character))?;
        let mut scratch = OutlineScratch::new();
        let bounds = self.outline_bounds(glyph, location, &mut scratch)?;
        (bounds.y_max > 0.0).then_some(bounds.y_max)
    }
}

fn typo_floats((ascender, descender, line_gap): (i16, i16, i16)) -> (f32, f32, f32) {
    (
        f32::from(ascender),
        f32::from(descender),
        f32::from(line_gap),
    )
}
