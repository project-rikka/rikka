use std::ops::Range;

use imara_diff::{Algorithm, Diff, InternedInput};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DiffHunk {
    pub(crate) base: Range<usize>,
    pub(crate) side: Range<usize>,
}

pub(crate) fn changes(base: &[&str], side: &[&str]) -> Vec<DiffHunk> {
    let mut input: InternedInput<&str> = InternedInput::default();
    input.update_before(base.iter().copied());
    input.update_after(side.iter().copied());

    let mut diff = Diff::compute(Algorithm::Histogram, &input);
    diff.postprocess_lines(&input);

    diff.hunks()
        .map(|hunk| DiffHunk {
            base: hunk.before.start as usize..hunk.before.end as usize,
            side: hunk.after.start as usize..hunk.after.end as usize,
        })
        .collect()
}
