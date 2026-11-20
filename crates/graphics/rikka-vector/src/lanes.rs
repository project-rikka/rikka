pub(crate) const LANES: usize = 16;

#[inline(always)]
pub(crate) fn slots<T: Copy + Default>(column: &mut Vec<T>, count: usize) -> &mut [T] {
    if column.len() < count {
        column.resize(count, T::default());
    }
    &mut column[..count]
}
