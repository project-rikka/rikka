use rikka_text_font::Transform;

#[derive(Debug, Default)]
pub(crate) struct TransformStack {
    base: Transform,
    pushed: Vec<Transform>,
}

impl TransformStack {
    pub(crate) fn reset(&mut self, base: Transform) {
        self.base = base;
        self.pushed.clear();
    }

    #[inline]
    pub(crate) fn current(&self) -> Transform {
        self.pushed.last().copied().unwrap_or(self.base)
    }

    pub(crate) fn push(&mut self, transform: Transform) {
        let combined = transform.then(self.current());
        self.pushed.push(combined);
    }

    pub(crate) fn pop(&mut self) {
        self.pushed.pop();
    }
}
