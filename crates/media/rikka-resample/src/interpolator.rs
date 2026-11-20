use rikka_simd::Simd;

use crate::cursor::Cursor;
use crate::sinc_table::SincTable;
use crate::weights::Weights;

pub(crate) struct Interpolator {
    table: SincTable,
    scratch: Vec<f32>,
}

impl Interpolator {
    pub(crate) fn new(table: SincTable) -> Interpolator {
        Interpolator {
            scratch: vec![0.0; table.span()],
            table,
        }
    }
}

impl Weights for Interpolator {
    #[inline(always)]
    fn weights<S: Simd>(&mut self, simd: S, cursor: &Cursor) -> &[f32] {
        self.table
            .phases(cursor.frac())
            .blend(simd, &mut self.scratch);
        &self.scratch
    }
}
