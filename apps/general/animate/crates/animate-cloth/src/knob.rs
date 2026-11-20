use rikka_physics_tailor::{ClothTuning, PieceKind, Preset};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Knob {
    Sway,
    Response,
    Weight,
}

impl Knob {
    pub const ALL: [Knob; 3] = [Knob::Sway, Knob::Response, Knob::Weight];

    #[must_use]
    pub fn id(self) -> &'static str {
        match self {
            Knob::Sway => "sway",
            Knob::Response => "response",
            Knob::Weight => "weight",
        }
    }

    #[must_use]
    pub fn named(id: &str) -> Option<Knob> {
        Knob::ALL.into_iter().find(|knob| knob.id() == id)
    }

    #[must_use]
    pub fn get(self, tuning: &ClothTuning) -> f32 {
        match self {
            Knob::Sway => tuning.sway,
            Knob::Response => tuning.response,
            Knob::Weight => tuning.weight,
        }
    }

    pub fn set(self, tuning: &mut ClothTuning, value: f32) {
        let slot = match self {
            Knob::Sway => &mut tuning.sway,
            Knob::Response => &mut tuning.response,
            Knob::Weight => &mut tuning.weight,
        };
        *slot = value.clamp(0.0, 1.0);
    }
}

#[must_use]
pub fn preset_id(preset: Preset) -> &'static str {
    match preset {
        Preset::Skirt => "skirt",
        Preset::Cape => "cape",
        Preset::Sleeve => "sleeve",
        Preset::Ribbon => "ribbon",
        Preset::Accessory => "accessory",
        Preset::LongHair => "long_hair",
        Preset::ShortHair => "short_hair",
        Preset::Bangs => "bangs",
        Preset::Tail => "tail",
    }
}

#[must_use]
pub fn preset_named(id: &str) -> Option<Preset> {
    Preset::ALL
        .into_iter()
        .find(|preset| preset_id(*preset) == id)
}

#[must_use]
pub fn kind_id(kind: PieceKind) -> &'static str {
    match kind {
        PieceKind::Sheet => "sheet",
        PieceKind::Strands => "strands",
    }
}
