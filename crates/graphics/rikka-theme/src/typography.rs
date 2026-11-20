#[derive(Clone, Debug, PartialEq)]
pub struct Typography {
    pub body_size: f32,
}

impl Default for Typography {
    fn default() -> Self {
        Self { body_size: 15.0 }
    }
}
