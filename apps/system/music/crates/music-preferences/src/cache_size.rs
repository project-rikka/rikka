use rikka_types::Choice;

const GIB: u64 = 1024 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum CacheSize {
    Off,
    Small,
    #[default]
    Medium,
    Large,
}

impl Choice for CacheSize {
    const ALL: &'static [CacheSize] = &[
        CacheSize::Off,
        CacheSize::Small,
        CacheSize::Medium,
        CacheSize::Large,
    ];

    fn slug(self) -> &'static str {
        match self {
            CacheSize::Off => "off",
            CacheSize::Small => "2g",
            CacheSize::Medium => "10g",
            CacheSize::Large => "50g",
        }
    }
}

impl CacheSize {
    pub fn bytes(self) -> u64 {
        match self {
            CacheSize::Off => 0,
            CacheSize::Small => 2 * GIB,
            CacheSize::Medium => 10 * GIB,
            CacheSize::Large => 50 * GIB,
        }
    }
}
