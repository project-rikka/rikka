use rikka_types::Choice;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Shelf {
    #[default]
    Models,
    Motions,
    Cameras,
    Stages,
    Accessories,
    Music,
}

impl Shelf {
    pub const ALL: [Shelf; 6] = [
        Shelf::Models,
        Shelf::Motions,
        Shelf::Cameras,
        Shelf::Stages,
        Shelf::Accessories,
        Shelf::Music,
    ];

    #[must_use]
    pub const fn folder(self) -> &'static str {
        match self {
            Shelf::Models => "Models",
            Shelf::Motions => "Motions",
            Shelf::Cameras => "Cameras",
            Shelf::Stages => "Stages",
            Shelf::Accessories => "Accessories",
            Shelf::Music => "Music",
        }
    }

    #[must_use]
    pub fn from_folder(name: &str) -> Option<Shelf> {
        let name = name.to_lowercase();
        let shelf = match name.as_str() {
            "models" | "model" => Shelf::Models,
            "motions" | "motion" => Shelf::Motions,
            "cameras" | "camera" => Shelf::Cameras,
            "stages" | "stage" | "background" => Shelf::Stages,
            "accessories" | "accessory" => Shelf::Accessories,
            "music" | "wave" | "sound" => Shelf::Music,
            _ => return None,
        };
        Some(shelf)
    }

    #[must_use]
    pub const fn index(self) -> usize {
        self as usize
    }

    #[must_use]
    pub const fn holds_models(self) -> bool {
        matches!(self, Shelf::Models | Shelf::Stages | Shelf::Accessories)
    }
}

impl Choice for Shelf {
    const ALL: &'static [Self] = &Shelf::ALL;

    fn slug(self) -> &'static str {
        match self {
            Shelf::Models => "models",
            Shelf::Motions => "motions",
            Shelf::Cameras => "cameras",
            Shelf::Stages => "stages",
            Shelf::Accessories => "accessories",
            Shelf::Music => "music",
        }
    }
}
