use glam::Vec3;

use crate::gather::gather;

#[derive(Clone, Debug, PartialEq)]
pub struct ShapeKey {
    pub name: String,
    pub positions: Vec<Vec3>,
    pub value: f32,
    pub min: f32,
    pub max: f32,
    pub relative_to: u32,
    pub vertex_group: Option<String>,
    pub muted: bool,
}

impl ShapeKey {
    pub fn new(name: impl Into<String>, positions: Vec<Vec3>) -> ShapeKey {
        ShapeKey {
            name: name.into(),
            positions,
            value: 0.0,
            min: 0.0,
            max: 1.0,
            relative_to: 0,
            vertex_group: None,
            muted: false,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ShapeKeys {
    pub keys: Vec<ShapeKey>,
    pub active: Option<usize>,
}

impl ShapeKeys {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.keys.len()
    }

    #[must_use]
    pub fn basis(&self) -> Option<&ShapeKey> {
        self.keys.first()
    }

    #[must_use]
    pub fn index(&self, name: &str) -> Option<u32> {
        self.keys
            .iter()
            .position(|key| key.name == name)
            .map(|index| index as u32)
    }

    #[must_use]
    pub fn get(&self, name: &str) -> Option<&ShapeKey> {
        self.keys.iter().find(|key| key.name == name)
    }

    pub fn add(&mut self, key: ShapeKey) -> u32 {
        self.keys.push(key);
        let index = self.keys.len() - 1;
        self.active.get_or_insert(index);
        index as u32
    }

    pub fn remove(&mut self, index: u32) {
        let index = index as usize;
        if index >= self.keys.len() {
            return;
        }
        self.keys.remove(index);
        for key in &mut self.keys {
            if key.relative_to as usize == index {
                key.relative_to = 0;
            } else if key.relative_to as usize > index {
                key.relative_to -= 1;
            }
        }
        self.active = match self.active {
            Some(active) if active == index => {
                (!self.keys.is_empty()).then(|| active.min(self.keys.len() - 1))
            }
            Some(active) if active > index => Some(active - 1),
            other => other,
        };
    }

    #[must_use]
    pub fn reference(&self, key: usize) -> Option<&ShapeKey> {
        self.keys.get(self.keys.get(key)?.relative_to as usize)
    }

    pub fn resize(&mut self, points: usize) {
        for key in &mut self.keys {
            key.positions.resize(points, Vec3::ZERO);
        }
    }

    pub fn gather(&mut self, indices: &[u32]) {
        for key in &mut self.keys {
            key.positions = gather(&key.positions, indices);
        }
    }

    pub fn copy(&mut self, from: usize, to: usize) {
        for key in &mut self.keys {
            key.positions[to] = key.positions[from];
        }
    }

    pub fn interpolate(&mut self, sources: &[(u32, f32)], to: usize) {
        for key in &mut self.keys {
            key.positions[to] = mix(&key.positions, sources);
        }
    }

    pub fn push_copy(&mut self, from: usize) {
        for key in &mut self.keys {
            let copied = key.positions[from];
            key.positions.push(copied);
        }
    }

    pub fn push_interpolated(&mut self, sources: &[(u32, f32)]) {
        for key in &mut self.keys {
            let mixed = mix(&key.positions, sources);
            key.positions.push(mixed);
        }
    }

    pub fn extend_from(&mut self, other: &ShapeKeys, positions: &[Vec3], other_positions: &[Vec3]) {
        if self.keys.is_empty() && other.keys.is_empty() {
            return;
        }
        for key in &mut self.keys {
            match other.get(&key.name) {
                Some(added) => key.positions.extend_from_slice(&added.positions),
                None => key.positions.extend_from_slice(other_positions),
            }
        }
        let first_added = self.keys.len();
        if self.keys.is_empty() {
            let mut basis = other.keys[0].clone();
            basis.positions = positions.to_vec();
            basis.positions.extend_from_slice(&other.keys[0].positions);
            self.keys.push(basis);
        }
        for added in &other.keys {
            if self.keys.iter().any(|key| key.name == added.name) {
                continue;
            }
            let mut key = added.clone();
            key.positions = positions.to_vec();
            key.positions.extend_from_slice(&added.positions);
            self.keys.push(key);
        }
        let names: Vec<String> = self.keys.iter().map(|key| key.name.clone()).collect();
        for key in &mut self.keys[first_added..] {
            let reference = other
                .keys
                .get(key.relative_to as usize)
                .map(|reference| reference.name.as_str());
            key.relative_to = reference
                .and_then(|name| names.iter().position(|candidate| candidate == name))
                .unwrap_or(0) as u32;
        }
        if self.active.is_none() {
            self.active = Some(0);
        }
    }
}

fn mix(positions: &[Vec3], sources: &[(u32, f32)]) -> Vec3 {
    sources
        .iter()
        .map(|&(index, weight)| positions[index as usize] * weight)
        .sum()
}
