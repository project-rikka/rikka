#[derive(Clone, Debug, PartialEq)]
pub struct LookAtCurves {
    pub horizontal_inner: Vec<f32>,
    pub horizontal_outer: Vec<f32>,
    pub vertical_down: Vec<f32>,
    pub vertical_up: Vec<f32>,
}

impl LookAtCurves {
    pub const LINEAR: [f32; 8] = [0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0, 0.0];
}

impl Default for LookAtCurves {
    fn default() -> Self {
        Self {
            horizontal_inner: Self::LINEAR.to_vec(),
            horizontal_outer: Self::LINEAR.to_vec(),
            vertical_down: Self::LINEAR.to_vec(),
            vertical_up: Self::LINEAR.to_vec(),
        }
    }
}
