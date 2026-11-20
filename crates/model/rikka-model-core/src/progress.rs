#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Stage {
    Reading,
    Parsing,
    Converting,
    Writing,
    Resources,
    Finishing,
}

impl std::fmt::Display for Stage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            Self::Reading => "reading",
            Self::Parsing => "parsing",
            Self::Converting => "converting",
            Self::Writing => "writing",
            Self::Resources => "resources",
            Self::Finishing => "finishing",
        };
        f.write_str(name)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Progress {
    pub stage: Stage,
    pub fraction: f32,
}

impl Progress {
    #[must_use]
    pub fn new(stage: Stage, fraction: f32) -> Self {
        Self {
            stage,
            fraction: fraction.clamp(0.0, 1.0),
        }
    }
}
