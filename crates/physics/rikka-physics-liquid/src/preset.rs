use crate::liquid::Liquid;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Preset {
    #[default]
    Water,
    Slime,
    Lotion,
    Honey,
    Milk,
    Blood,
}

const CLEAR_RUNNY_WATER: Liquid = Liquid {
    viscosity: 0.0,
    elasticity: 0.0,
    relaxation: 1.0,
    cohesion: 0.4,
    adhesion: 0.05,
    friction: 0.5,
    spray: 0.5,
    wetting: 0.5,
    color: [0.78, 0.9, 0.96],
    depth: 0.12,
    cloudiness: 0.2,
    roughness: 0.02,
    ior: 1.333,
    tint: [1.0, 1.0, 1.0],
};

const STRETCHY_GREEN_SLIME: Liquid = Liquid {
    viscosity: 0.55,
    elasticity: 0.55,
    relaxation: 0.15,
    cohesion: 0.8,
    adhesion: 0.8,
    friction: 0.8,
    spray: 0.0,
    wetting: 0.3,
    color: [0.3, 0.9, 0.35],
    depth: 0.05,
    cloudiness: 0.15,
    roughness: 0.08,
    ior: 1.35,
    tint: [0.6, 0.9, 0.6],
};

const SLIPPERY_STRINGY_LOTION: Liquid = Liquid {
    viscosity: 0.6,
    elasticity: 0.45,
    relaxation: 0.7,
    cohesion: 0.7,
    adhesion: 0.85,
    friction: 0.7,
    spray: 0.0,
    wetting: 0.8,
    color: [0.95, 0.95, 0.93],
    depth: 0.01,
    cloudiness: 0.9,
    roughness: 0.15,
    ior: 1.4,
    tint: [0.97, 0.97, 0.95],
};

const THICK_AMBER_HONEY: Liquid = Liquid {
    viscosity: 0.78,
    elasticity: 0.0,
    relaxation: 1.0,
    cohesion: 0.5,
    adhesion: 0.6,
    friction: 0.6,
    spray: 0.0,
    wetting: 0.6,
    color: [0.9, 0.45, 0.05],
    depth: 0.05,
    cloudiness: 0.05,
    roughness: 0.05,
    ior: 1.49,
    tint: [0.85, 0.55, 0.15],
};

const RUNNY_OPAQUE_MILK: Liquid = Liquid {
    viscosity: 0.2,
    elasticity: 0.0,
    relaxation: 1.0,
    cohesion: 0.4,
    adhesion: 0.15,
    friction: 0.5,
    spray: 0.4,
    wetting: 0.7,
    color: [0.95, 0.93, 0.88],
    depth: 0.005,
    cloudiness: 1.0,
    roughness: 0.1,
    ior: 1.35,
    tint: [0.95, 0.94, 0.9],
};

const STAINING_DARK_RED_BLOOD: Liquid = Liquid {
    viscosity: 0.35,
    elasticity: 0.05,
    relaxation: 0.9,
    cohesion: 0.35,
    adhesion: 0.4,
    friction: 0.3,
    spray: 0.2,
    wetting: 1.0,
    color: [0.6, 0.02, 0.02],
    depth: 0.003,
    cloudiness: 0.6,
    roughness: 0.08,
    ior: 1.36,
    tint: [0.45, 0.02, 0.02],
};

impl Preset {
    pub const ALL: [Preset; 6] = [
        Self::Water,
        Self::Slime,
        Self::Lotion,
        Self::Honey,
        Self::Milk,
        Self::Blood,
    ];

    #[must_use]
    pub fn id(self) -> &'static str {
        match self {
            Self::Water => "water",
            Self::Slime => "slime",
            Self::Lotion => "lotion",
            Self::Honey => "honey",
            Self::Milk => "milk",
            Self::Blood => "blood",
        }
    }

    #[must_use]
    pub fn named(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|preset| preset.id() == id)
    }

    #[must_use]
    pub fn liquid(self) -> Liquid {
        match self {
            Self::Water => CLEAR_RUNNY_WATER,
            Self::Slime => STRETCHY_GREEN_SLIME,
            Self::Lotion => SLIPPERY_STRINGY_LOTION,
            Self::Honey => THICK_AMBER_HONEY,
            Self::Milk => RUNNY_OPAQUE_MILK,
            Self::Blood => STAINING_DARK_RED_BLOOD,
        }
    }
}
