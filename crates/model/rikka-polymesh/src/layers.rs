#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Layers {
    pub names: Vec<String>,
    pub active: Option<usize>,
    pub render: Option<usize>,
}

impl Layers {
    #[must_use]
    pub fn len(&self) -> usize {
        self.names.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.names.is_empty()
    }

    #[must_use]
    pub fn contains(&self, name: &str) -> bool {
        self.position(name).is_some()
    }

    #[must_use]
    pub fn position(&self, name: &str) -> Option<usize> {
        self.names.iter().position(|layer| layer == name)
    }

    #[must_use]
    pub fn active_name(&self) -> Option<&str> {
        self.active
            .and_then(|index| self.names.get(index))
            .map(String::as_str)
    }

    #[must_use]
    pub fn render_name(&self) -> Option<&str> {
        self.render
            .and_then(|index| self.names.get(index))
            .map(String::as_str)
    }

    pub fn push(&mut self, name: impl Into<String>) -> usize {
        let name = name.into();
        if let Some(index) = self.position(&name) {
            return index;
        }
        self.names.push(name);
        let index = self.names.len() - 1;
        self.active.get_or_insert(index);
        self.render.get_or_insert(index);
        index
    }

    pub fn remove(&mut self, name: &str) -> bool {
        let Some(index) = self.position(name) else {
            return false;
        };
        self.names.remove(index);
        let shift = |slot: &mut Option<usize>, names: usize| {
            *slot = match *slot {
                Some(current) if current == index => (names > 0).then(|| index.min(names - 1)),
                Some(current) if current > index => Some(current - 1),
                other => other,
            };
        };
        shift(&mut self.active, self.names.len());
        shift(&mut self.render, self.names.len());
        true
    }

    pub fn rename(&mut self, from: &str, to: &str) {
        if let Some(index) = self.position(from) {
            to.clone_into(&mut self.names[index]);
        }
    }

    pub fn set_active(&mut self, name: &str) -> bool {
        let index = self.position(name);
        if index.is_some() {
            self.active = index;
        }
        index.is_some()
    }

    pub fn set_render(&mut self, name: &str) -> bool {
        let index = self.position(name);
        if index.is_some() {
            self.render = index;
        }
        index.is_some()
    }

    pub(crate) fn union(&mut self, other: &Layers) {
        let was_empty = self.names.is_empty();
        for name in &other.names {
            self.push(name.clone());
        }
        if was_empty {
            self.active = other.active_name().and_then(|name| self.position(name));
            self.render = other.render_name().and_then(|name| self.position(name));
        }
    }

    pub(crate) fn retain(&mut self, mut keep: impl FnMut(&str) -> bool) {
        let gone: Vec<String> = self
            .names
            .iter()
            .filter(|name| !keep(name))
            .cloned()
            .collect();
        for name in gone {
            self.remove(&name);
        }
    }

    pub(crate) fn clamp(&mut self) -> bool {
        let names = self.names.len();
        let mut repaired = false;
        for slot in [&mut self.active, &mut self.render] {
            if slot.is_some_and(|index| index >= names) {
                *slot = (names > 0).then_some(0);
                repaired = true;
            }
        }
        repaired
    }
}
