use smallvec::SmallVec;

use rikka_wayland_wire::{ClientId, Resource};

#[derive(Debug)]
pub struct ByClient<T> {
    clients: Vec<ClientId>,
    objects: Vec<SmallVec<[T; 2]>>,
}

impl<T> Default for ByClient<T> {
    fn default() -> Self {
        Self {
            clients: Vec::new(),
            objects: Vec::new(),
        }
    }
}

impl<T: Resource> ByClient<T> {
    pub fn add(&mut self, object: T) {
        let client = object.object().client();
        match self.slot(client) {
            Some(slot) => self.objects[slot].push(object),
            None => {
                self.clients.push(client);
                self.objects.push(SmallVec::from_elem(object, 1));
            }
        }
    }

    pub fn remove(&mut self, object: &T) {
        let Some(slot) = self.slot(object.object().client()) else {
            return;
        };
        let objects = &mut self.objects[slot];
        objects.retain(|stored| stored != object);
        if objects.is_empty() {
            self.clients.swap_remove(slot);
            self.objects.swap_remove(slot);
        }
    }

    pub fn for_client(&self, client: ClientId) -> &[T] {
        self.slot(client)
            .map_or(&[], |slot| self.objects[slot].as_slice())
    }

    pub fn all(&self) -> impl Iterator<Item = &T> {
        self.objects.iter().flatten()
    }

    fn slot(&self, client: ClientId) -> Option<usize> {
        self.clients
            .iter()
            .position(|known_client| *known_client == client)
    }
}
