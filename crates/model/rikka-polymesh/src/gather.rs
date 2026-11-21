use crate::PARALLEL;

const GRAIN: usize = 1 << 14;

pub(crate) fn gather<T: Copy + Send + Sync>(source: &[T], indices: &[u32]) -> Vec<T> {
    let Some(&seed) = source.first() else {
        return indices
            .iter()
            .map(|&index| source[index as usize])
            .collect();
    };
    let mut output = vec![seed; indices.len()];
    PARALLEL.grain(GRAIN).fill(&mut output, |index, slot| {
        *slot = source[indices[index] as usize]
    });
    output
}
