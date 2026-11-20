use std::time::SystemTime;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Accuracy {
    Country,
    #[default]
    City,
    Neighborhood,
    Street,
    Exact,
}

impl Accuracy {
    pub(crate) fn level(self) -> u32 {
        match self {
            Accuracy::Country => 1,
            Accuracy::City => 4,
            Accuracy::Neighborhood => 5,
            Accuracy::Street => 6,
            Accuracy::Exact => 8,
        }
    }

    pub(crate) fn threshold(self) -> u32 {
        match self {
            Accuracy::Country => 50_000,
            Accuracy::City => 5_000,
            Accuracy::Neighborhood => 1_000,
            Accuracy::Street => 100,
            Accuracy::Exact => 0,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Fix {
    pub latitude: f64,
    pub longitude: f64,
    pub accuracy: f64,
    pub altitude: Option<f64>,
    pub description: Option<String>,
    pub at: SystemTime,
}
