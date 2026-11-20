use rikka_types::Size;

use crate::tracked::Tracked;

#[derive(Clone, Debug)]
pub struct Measured(Tracked<Size>);

impl Measured {
    #[must_use]
    pub fn new(size: Size) -> Measured {
        Measured(Tracked::new(size))
    }

    #[must_use]
    pub fn get(&self) -> Size {
        self.0.get()
    }

    pub fn set(&self, size: Size) -> bool {
        self.0.set(size)
    }

    pub fn take_moved(&self) -> bool {
        self.0.take_moved()
    }
}
