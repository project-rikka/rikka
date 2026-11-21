use crate::PARALLEL;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct VertexGroup {
    pub name: String,
    pub locked: bool,
}

impl VertexGroup {
    pub fn new(name: impl Into<String>) -> VertexGroup {
        VertexGroup {
            name: name.into(),
            locked: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Influence {
    pub group: u32,
    pub weight: f32,
}

pub type Influences = Vec<Influence>;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct VertexGroups {
    pub groups: Vec<VertexGroup>,
    pub weights: Vec<Influences>,
    pub active: Option<usize>,
}

impl VertexGroups {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.groups.is_empty()
    }

    #[must_use]
    pub fn has_weights(&self) -> bool {
        !self.weights.is_empty()
    }

    #[must_use]
    pub fn index(&self, name: &str) -> Option<u32> {
        self.groups
            .iter()
            .position(|group| group.name == name)
            .map(|index| index as u32)
    }

    pub fn add(&mut self, name: impl Into<String>) -> u32 {
        let name = name.into();
        if let Some(index) = self.index(&name) {
            return index;
        }
        self.groups.push(VertexGroup::new(name));
        let index = self.groups.len() - 1;
        self.active.get_or_insert(index);
        index as u32
    }

    #[must_use]
    pub fn weight(&self, point: usize, group: u32) -> f32 {
        self.weights
            .get(point)
            .and_then(|influences| influences.iter().find(|influence| influence.group == group))
            .map_or(0.0, |influence| influence.weight)
    }

    pub fn ensure_weights(&mut self, points: usize) {
        if self.weights.len() != points {
            self.weights.resize(points, Vec::new());
        }
    }

    pub fn set_weight(&mut self, point: usize, group: u32, weight: f32) {
        let influences = &mut self.weights[point];
        match influences
            .iter_mut()
            .find(|influence| influence.group == group)
        {
            Some(influence) => influence.weight = weight,
            None => influences.push(Influence { group, weight }),
        }
    }

    pub fn clear_weight(&mut self, point: usize, group: u32) {
        if let Some(influences) = self.weights.get_mut(point) {
            influences.retain(|influence| influence.group != group);
        }
    }

    pub fn remove_group(&mut self, group: u32) {
        if group as usize >= self.groups.len() {
            return;
        }
        self.groups.remove(group as usize);
        for influences in &mut self.weights {
            influences.retain(|influence| influence.group != group);
            for influence in influences.iter_mut() {
                if influence.group > group {
                    influence.group -= 1;
                }
            }
        }
        self.active = match self.active {
            Some(active) if active == group as usize => {
                (!self.groups.is_empty()).then(|| active.min(self.groups.len() - 1))
            }
            Some(active) if active > group as usize => Some(active - 1),
            other => other,
        };
    }

    pub fn resize(&mut self, points: usize) {
        if self.has_weights() {
            self.weights.resize(points, Vec::new());
        }
    }

    pub fn gather(&mut self, indices: &[u32]) {
        if self.has_weights() {
            let weights = &self.weights;
            self.weights = PARALLEL.grain(1 << 13).collect(indices.len(), |index| {
                weights[indices[index] as usize].clone()
            });
        }
    }

    pub fn copy(&mut self, from: usize, to: usize) {
        if self.has_weights() {
            self.weights[to] = self.weights[from].clone();
        }
    }

    pub fn interpolate(&mut self, sources: &[(u32, f32)], to: usize) {
        if self.has_weights() {
            self.weights[to] = self.mixed(sources);
        }
    }

    pub fn push_copy(&mut self, from: usize) {
        if self.has_weights() {
            let copied = self.weights[from].clone();
            self.weights.push(copied);
        }
    }

    pub fn push_interpolated(&mut self, sources: &[(u32, f32)]) {
        if self.has_weights() {
            let mixed = self.mixed(sources);
            self.weights.push(mixed);
        }
    }

    #[must_use]
    pub fn mixed(&self, sources: &[(u32, f32)]) -> Influences {
        let mut mixed: Influences = Vec::new();
        for &(point, share) in sources {
            for influence in &self.weights[point as usize] {
                let weight = influence.weight * share;
                if weight == 0.0 {
                    continue;
                }
                match mixed
                    .iter_mut()
                    .find(|existing| existing.group == influence.group)
                {
                    Some(existing) => existing.weight += weight,
                    None => mixed.push(Influence {
                        group: influence.group,
                        weight,
                    }),
                }
            }
        }
        mixed
    }

    pub fn extend_from(&mut self, other: &VertexGroups, points: usize, other_points: usize) {
        let map: Vec<u32> = other
            .groups
            .iter()
            .map(|group| {
                let index = self.add(group.name.clone());
                self.groups[index as usize].locked |= group.locked;
                index
            })
            .collect();
        if !self.has_weights() && !other.has_weights() {
            return;
        }
        self.weights.resize(points, Vec::new());
        match other.has_weights() {
            true => self.weights.extend(other.weights.iter().map(|influences| {
                influences
                    .iter()
                    .filter_map(|influence| {
                        Some(Influence {
                            group: *map.get(influence.group as usize)?,
                            weight: influence.weight,
                        })
                    })
                    .collect::<Influences>()
            })),
            false => self.weights.resize(points + other_points, Vec::new()),
        }
    }
}
