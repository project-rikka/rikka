#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct User {
    pub name: String,
    pub app_id: Option<String>,
    pub pid: Option<u32>,
    pub icon: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Sensor {
    Microphone,
    Camera,
    Screen,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct State {
    pub microphone: Vec<User>,
    pub camera: Vec<User>,
    pub screen: Vec<User>,
}

impl State {
    #[must_use]
    pub fn users(&self, sensor: Sensor) -> &[User] {
        match sensor {
            Sensor::Microphone => &self.microphone,
            Sensor::Camera => &self.camera,
            Sensor::Screen => &self.screen,
        }
    }

    #[must_use]
    pub fn in_use(&self, sensor: Sensor) -> bool {
        !self.users(sensor).is_empty()
    }

    #[must_use]
    pub fn is_quiet(&self) -> bool {
        self.microphone.is_empty() && self.camera.is_empty() && self.screen.is_empty()
    }
}

pub(crate) fn deduplicated(mut users: Vec<User>) -> Vec<User> {
    users.sort();
    users.dedup();
    users
}
