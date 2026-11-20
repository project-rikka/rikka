#[derive(Clone, Debug)]
pub(crate) struct Ring<T> {
    pub(crate) buffer: Vec<T>,
    pub(crate) write: usize,
    pub(crate) read: usize,
}

impl<T: Clone> Ring<T> {
    pub(crate) fn new(len: usize, item: T) -> Ring<T> {
        Ring {
            buffer: vec![item; len],
            write: 0,
            read: 0,
        }
    }
}

impl<T> Ring<T> {
    pub(crate) fn len(&self) -> usize {
        self.buffer.len()
    }

    pub(crate) fn inc(&self, index: usize) -> usize {
        if index + 1 < self.len() { index + 1 } else { 0 }
    }

    pub(crate) fn dec(&self, index: usize) -> usize {
        if index > 0 { index - 1 } else { self.len() - 1 }
    }

    pub(crate) fn offset(&self, index: usize, offset: isize) -> usize {
        let len = self.len() as isize;
        (len + index as isize + offset).rem_euclid(len) as usize
    }

    pub(crate) fn update_write(&mut self, offset: isize) {
        self.write = self.offset(self.write, offset);
    }

    pub(crate) fn inc_write(&mut self) {
        self.write = self.inc(self.write);
    }

    pub(crate) fn dec_write(&mut self) {
        self.write = self.dec(self.write);
    }

    pub(crate) fn update_read(&mut self, offset: isize) {
        self.read = self.offset(self.read, offset);
    }

    pub(crate) fn inc_read(&mut self) {
        self.read = self.inc(self.read);
    }

    pub(crate) fn dec_read(&mut self) {
        self.read = self.dec(self.read);
    }
}
