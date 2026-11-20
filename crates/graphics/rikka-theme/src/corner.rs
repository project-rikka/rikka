use rikka_types::CornerRadii;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Corner {
    Window,
    Panel,
    Control,
    Item,
    Detail,
}

impl Corner {
    #[must_use]
    pub const fn radius(self) -> f32 {
        match self {
            Corner::Window => 16.0,
            Corner::Panel => 14.0,
            Corner::Control => 10.0,
            Corner::Item => 8.0,
            Corner::Detail => 4.0,
        }
    }

    #[must_use]
    pub const fn nested(self, inset: f32) -> f32 {
        (self.radius() - inset).max(Corner::Detail.radius().min(self.radius()))
    }
}

impl From<Corner> for CornerRadii {
    fn from(corner: Corner) -> Self {
        CornerRadii::uniform(corner.radius())
    }
}
