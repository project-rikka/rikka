use rikka_ui::Observers;

use crate::ids::ProfileId;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Profile {
    pub id: ProfileId,
    pub name: String,
}

#[derive(Debug)]
pub struct Profiles {
    list: Vec<Profile>,
    observers: Observers,
}

impl Profiles {
    pub fn new(list: Vec<Profile>) -> Profiles {
        Profiles {
            list,
            observers: Observers::new(),
        }
    }

    pub fn list(&self) -> &[Profile] {
        self.observers.read();
        &self.list
    }

    pub fn get(&self, id: ProfileId) -> Option<&Profile> {
        self.observers.read();
        self.list.iter().find(|profile| profile.id == id)
    }

    pub fn add(&mut self, profile: Profile) {
        self.list.push(profile);
        self.observers.changed();
    }

    pub fn remove(&mut self, id: ProfileId) {
        let before = self.list.len();
        self.list.retain(|profile| profile.id != id);
        if self.list.len() != before {
            self.observers.changed();
        }
    }

    pub fn rename(&mut self, id: ProfileId, name: String) {
        if let Some(profile) = self.list.iter_mut().find(|profile| profile.id == id)
            && profile.name != name
        {
            profile.name = name;
            self.observers.changed();
        }
    }
}
