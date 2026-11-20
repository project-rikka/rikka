pub(crate) struct PendingUploads<T> {
    pending: Vec<T>,
    sent: Vec<T>,
}

impl<T> Default for PendingUploads<T> {
    fn default() -> Self {
        Self {
            pending: Vec::new(),
            sent: Vec::new(),
        }
    }
}

impl<T> PendingUploads<T> {
    pub(crate) fn push(&mut self, upload: T) {
        self.pending.push(upload);
    }

    pub(crate) fn pending(&self) -> &[T] {
        &self.pending
    }

    pub(crate) fn pending_mut(&mut self) -> &mut [T] {
        &mut self.pending
    }

    pub(crate) fn unsettled(&self) -> impl Iterator<Item = &T> {
        self.pending.iter().chain(&self.sent)
    }

    pub(crate) fn mark_sent(&mut self) {
        self.sent.append(&mut self.pending);
    }

    pub(crate) fn commit(&mut self) {
        self.sent.clear();
    }

    pub(crate) fn revert(&mut self) {
        let later = std::mem::take(&mut self.pending);
        self.pending = std::mem::take(&mut self.sent);
        self.pending.extend(later);
    }
}
