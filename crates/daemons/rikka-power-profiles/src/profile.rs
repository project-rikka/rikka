#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Profile {
    PowerSaver,
    Balanced,
    Performance,
}

impl Profile {
    pub const ALL: [Profile; 3] = [Profile::PowerSaver, Profile::Balanced, Profile::Performance];

    #[must_use]
    pub fn named(name: &str) -> Option<Profile> {
        Profile::ALL
            .into_iter()
            .find(|profile| profile.name() == name)
    }

    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Profile::PowerSaver => "power-saver",
            Profile::Balanced => "balanced",
            Profile::Performance => "performance",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Degraded {
    LapDetected,
    HighTemperature,
    Other(String),
}

impl Degraded {
    pub(crate) fn named(reason: &str) -> Option<Degraded> {
        match reason {
            "" => None,
            "lap-detected" => Some(Degraded::LapDetected),
            "high-operating-temperature" => Some(Degraded::HighTemperature),
            other => Some(Degraded::Other(other.to_owned())),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hold {
    pub application: String,
    pub profile: Profile,
    pub reason: String,
}
