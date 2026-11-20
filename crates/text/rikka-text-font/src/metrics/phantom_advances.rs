use crate::glyf::loca::Glyf;
use crate::parse::f2dot14::F2Dot14;
use crate::sfnt::font::Font;
use crate::sfnt::glyph_id::GlyphId;
use crate::variations::fixed::Fixed;
use crate::variations::gvar::Gvar;

const LEFT: usize = 0;
const RIGHT: usize = 1;

#[derive(Clone, Copy, Debug)]
pub struct PhantomAdvances<'a, 'c> {
    gvar: Gvar<'a>,
    glyf: Glyf<'a>,
    coords: &'c [F2Dot14],
}

impl PhantomAdvances<'_, '_> {
    pub fn advance_delta_fixed(&self, glyph: GlyphId) -> i32 {
        let Some((source, point_count)) = self.glyf.metrics_source(glyph) else {
            return 0;
        };
        let Some(tuples) = self.gvar.fixed_tuples(source, self.coords) else {
            return 0;
        };
        let mut phantoms = [Fixed::ZERO; 2];
        for tuple in tuples {
            tuple.add_horizontal_phantoms(point_count, &mut phantoms);
        }
        phantoms[RIGHT].rounded() - phantoms[LEFT].rounded()
    }
}

impl<'a> Font<'a> {
    pub fn phantom_advances<'c>(&self, coords: &'c [F2Dot14]) -> Option<PhantomAdvances<'a, 'c>> {
        Some(PhantomAdvances {
            gvar: self.gvar()?,
            glyf: self.glyf()?,
            coords,
        })
    }
}
