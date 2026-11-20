use crate::PointerMode;
use crate::tracked::Tracked;

#[derive(Clone, Debug)]
pub struct Reach(Tracked<PointerMode>);

impl Reach {
    #[must_use]
    pub fn new(mode: PointerMode) -> Reach {
        Reach(Tracked::new(mode))
    }

    #[must_use]
    pub fn get(&self) -> PointerMode {
        self.0.get()
    }

    pub fn set(&self, mode: PointerMode) -> bool {
        self.0.set(mode)
    }

    pub fn take_moved(&self) -> bool {
        self.0.take_moved()
    }
}
