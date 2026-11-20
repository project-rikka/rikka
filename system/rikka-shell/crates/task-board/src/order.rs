#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Order {
    groups: Vec<(usize, String)>,
    windows: Vec<String>,
}

impl Order {
    pub(crate) fn settle_groups<'a>(&mut self, present: impl Iterator<Item = (usize, &'a str)>) {
        let present: Vec<(usize, &str)> = present.collect();
        self.groups
            .retain(|(space, key)| present.contains(&(*space, key.as_str())));
        for (space, key) in present {
            let known =
                |(known_space, known): &(usize, String)| *known_space == space && known == key;
            if !self.groups.iter().any(known) {
                self.groups.push((space, key.to_owned()));
            }
        }
    }

    pub(crate) fn settle_windows<'a>(&mut self, present: impl Iterator<Item = &'a str>) {
        let present: Vec<&str> = present.collect();
        self.windows.retain(|id| present.contains(&id.as_str()));
        for id in present {
            if !self.windows.iter().any(|known| known == id) {
                self.windows.push(id.to_owned());
            }
        }
    }

    pub(crate) fn group_place(&self, space: usize, key: &str) -> usize {
        self.groups
            .iter()
            .position(|(known_space, known)| *known_space == space && known == key)
            .unwrap_or(usize::MAX)
    }

    pub(crate) fn window_place(&self, id: &str) -> usize {
        self.windows
            .iter()
            .position(|known| known == id)
            .unwrap_or(usize::MAX)
    }
}
