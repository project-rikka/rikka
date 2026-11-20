use std::collections::HashMap;
use std::hash::Hash;

#[derive(Debug)]
pub(crate) struct TextureRefs<T> {
    holders: HashMap<T, usize>,
    retired: Vec<T>,
}

impl<T> Default for TextureRefs<T> {
    fn default() -> Self {
        Self {
            holders: HashMap::new(),
            retired: Vec::new(),
        }
    }
}

impl<T: Copy + Eq + Hash> TextureRefs<T> {
    pub(crate) fn keep(&mut self, textures: impl IntoIterator<Item = T>) {
        for texture in textures {
            *self.holders.entry(texture).or_default() += 1;
        }
    }

    pub(crate) fn release(&mut self, textures: impl IntoIterator<Item = T>) -> Vec<T> {
        let mut freed = Vec::new();
        for texture in textures {
            let Some(holders) = self.holders.get_mut(&texture) else {
                continue;
            };
            *holders -= 1;
            if *holders > 0 {
                continue;
            }
            self.holders.remove(&texture);
            if let Some(index) = self.retired.iter().position(|retired| *retired == texture) {
                freed.push(self.retired.swap_remove(index));
            }
        }
        freed
    }

    pub(crate) fn holds(&self, texture: T) -> bool {
        self.holders.contains_key(&texture)
    }

    pub(crate) fn outlive(&mut self, texture: T) -> bool {
        let is_held = self.holds(texture);
        if is_held && !self.retired.contains(&texture) {
            self.retired.push(texture);
        }
        is_held
    }
}
