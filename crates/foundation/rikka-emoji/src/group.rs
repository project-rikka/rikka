#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub enum Group {
    Smileys,
    People,
    Nature,
    Food,
    Travel,
    Activities,
    Objects,
    Symbols,
    Flags,
}

impl Group {
    pub const ALL: [Group; 9] = [
        Group::Smileys,
        Group::People,
        Group::Nature,
        Group::Food,
        Group::Travel,
        Group::Activities,
        Group::Objects,
        Group::Symbols,
        Group::Flags,
    ];

    #[must_use]
    pub fn from_slug(slug: &str) -> Option<Group> {
        Group::ALL.into_iter().find(|group| group.slug() == slug)
    }

    #[must_use]
    pub fn slug(self) -> &'static str {
        match self {
            Group::Smileys => "smileys",
            Group::People => "people",
            Group::Nature => "nature",
            Group::Food => "food",
            Group::Travel => "travel",
            Group::Activities => "activities",
            Group::Objects => "objects",
            Group::Symbols => "symbols",
            Group::Flags => "flags",
        }
    }
}
