use crate::part::PartValues;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Slope {
    pub arriving: f32,
    pub leaving: f32,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Slopes {
    pub scalars: Vec<Slope>,
    pub turn: Option<Slope>,
}

impl Slopes {
    #[must_use]
    pub fn fits(&self, values: &PartValues) -> bool {
        self.scalars.len() == values.scalar_count()
            && self.turn.is_some() == values.turn().is_some()
    }
}
