use crate::Placement;
use crate::tracked::Tracked;

#[derive(Clone, Debug, Default)]
pub struct Outputs(Tracked<Vec<Placement>>);

impl Outputs {
    #[must_use]
    pub fn all(&self) -> Vec<Placement> {
        self.0.get()
    }

    pub fn set(&self, places: Vec<Placement>) -> bool {
        self.0.set(places)
    }
}
