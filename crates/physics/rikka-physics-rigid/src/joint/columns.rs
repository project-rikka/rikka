use rikka_physics_math::Wide;

use crate::solver::lanes::{MAX_LANES, transpose, untranspose};

#[inline(always)]
fn chunk<W: Wide>(rows: &[&[f32]; MAX_LANES], start: usize) -> [W; 4] {
    let mut pointers = [std::ptr::null::<f32>(); MAX_LANES];
    for (pointer, row) in pointers.iter_mut().zip(rows).take(W::LANES) {
        *pointer = row[start..start + 4].as_ptr();
    }
    unsafe { transpose::<W>(&pointers, 0) }
}

#[inline(always)]
pub(crate) fn columns<W: Wide, const N: usize>(rows: &[&[f32]; MAX_LANES], start: usize) -> [W; N] {
    const { assert!(N >= 4) };
    let mut out = [W::default(); N];
    let mut offset = 0;
    while offset < N {
        let at = offset.min(N - 4);
        out[at..at + 4].copy_from_slice(&chunk::<W>(rows, start + at));
        offset += 4;
    }
    out
}

#[inline(always)]
pub(crate) fn columns_from<W: Wide, const N: usize>(
    rows: &[&[f32]; MAX_LANES],
    first: usize,
) -> [W; N] {
    const { assert!(N <= 4) };
    let start = first.min(rows[0].len() - 4);
    let chunk = chunk::<W>(rows, start);
    let mut out = [W::default(); N];
    out.copy_from_slice(&chunk[first - start..first - start + N]);
    out
}

#[inline(always)]
pub(crate) unsafe fn store_rows<W: Wide, const N: usize>(
    columns: &[W; N],
    rows: &[*mut f32; MAX_LANES],
) {
    const { assert!(N >= 4) };
    let mut offset = 0;
    while offset < N {
        let at = offset.min(N - 4);
        unsafe {
            untranspose(
                [
                    columns[at],
                    columns[at + 1],
                    columns[at + 2],
                    columns[at + 3],
                ],
                rows,
                at,
            );
        }
        offset += 4;
    }
}
