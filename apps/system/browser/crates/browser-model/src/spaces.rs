use rikka_ui::Observers;

use browser_store::SpaceFields;

use crate::ids::{ProfileId, SpaceId};
use crate::look::SpaceIcon;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Space {
    pub id: SpaceId,
    pub name: String,
    pub icon: SpaceIcon,
    pub profile: ProfileId,
}

impl Space {
    pub fn from_store(id: SpaceId, fields: &SpaceFields) -> Space {
        Space {
            id,
            name: fields.name.clone(),
            icon: SpaceIcon::from_store(&fields.icon),
            profile: fields.profile,
        }
    }

    pub fn to_fields(&self) -> SpaceFields {
        SpaceFields {
            name: self.name.clone(),
            icon: self.icon.to_store(),
            profile: self.profile,
        }
    }
}

#[derive(Debug)]
pub struct Spaces {
    list: Vec<Space>,
    observers: Observers,
}

impl Spaces {
    pub fn new(list: Vec<Space>) -> Spaces {
        Spaces {
            list,
            observers: Observers::new(),
        }
    }

    pub fn list(&self) -> &[Space] {
        self.observers.read();
        &self.list
    }

    pub fn get(&self, id: SpaceId) -> Option<&Space> {
        self.list().iter().find(|space| space.id == id)
    }

    pub fn position(&self, id: SpaceId) -> Option<usize> {
        self.list().iter().position(|space| space.id == id)
    }

    pub fn first(&self) -> SpaceId {
        self.list()[0].id
    }

    pub fn step(&self, from: SpaceId, by: isize) -> Option<SpaceId> {
        let at = self.position(from)?;
        let to = at.checked_add_signed(by)?;
        self.list().get(to).map(|space| space.id)
    }

    pub fn add(&mut self, space: Space) {
        self.list.push(space);
        self.observers.changed();
    }

    pub fn remove(&mut self, id: SpaceId) -> Option<Space> {
        if self.list.len() <= 1 {
            return None;
        }
        let at = self.list.iter().position(|space| space.id == id)?;
        self.observers.changed();
        Some(self.list.remove(at))
    }

    pub fn edit(&mut self, id: SpaceId, edit: impl FnOnce(&mut Space)) {
        let Some(space) = self.list.iter_mut().find(|space| space.id == id) else {
            return;
        };
        let before = space.clone();
        edit(space);
        if *space != before {
            self.observers.changed();
        }
    }
}
