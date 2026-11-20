use crate::layout::apply_context::ApplyContext;
use crate::layout::matching::AnyGlyph;

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct PairSecond {
    found: Option<Result<usize, usize>>,
}

impl PairSecond {
    #[inline]
    pub(crate) fn find(&mut self, ctx: &ApplyContext<'_>) -> Result<usize, usize> {
        *self.found.get_or_insert_with(|| {
            let mut iterator = ctx.input_iterator();
            iterator.reset(ctx.buffer, ctx.buffer.idx);
            let mut unsafe_to = 0;
            if iterator.next(ctx.buffer, ctx.font, &AnyGlyph, &mut unsafe_to) {
                Ok(iterator.index)
            } else {
                Err(unsafe_to)
            }
        })
    }
}
