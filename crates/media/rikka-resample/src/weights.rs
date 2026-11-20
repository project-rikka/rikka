use rikka_simd::Simd;

use crate::cursor::Cursor;

pub(crate) trait Weights {
    fn weights<S: Simd>(&mut self, simd: S, cursor: &Cursor) -> &[f32];
}
