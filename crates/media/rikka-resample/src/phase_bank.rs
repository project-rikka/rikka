use rikka_simd::Simd;

use crate::cursor::Cursor;
use crate::sinc_table::SincTable;
use crate::weights::Weights;

pub(crate) struct PhaseBank {
    rows: Vec<f32>,
    span: usize,
}

impl PhaseBank {
    #[inline(always)]
    pub(crate) fn new<S: Simd>(simd: S, table: &SincTable, phases: usize) -> PhaseBank {
        let span = table.span();
        let mut rows = vec![0.0; phases * span];
        for (phase, row) in rows.chunks_exact_mut(span).enumerate() {
            table.phases(phase as f64 / phases as f64).blend(simd, row);
        }
        PhaseBank { rows, span }
    }
}

impl Weights for PhaseBank {
    #[inline(always)]
    fn weights<S: Simd>(&mut self, _: S, cursor: &Cursor) -> &[f32] {
        &self.rows[cursor.phase() * self.span..][..self.span]
    }
}
