use animate_body::Faces;
use rustc_hash::FxHashMap;

use crate::morphs::MorphKey;

pub(super) struct Adopting<'f> {
    faces: &'f Faces,
    shares: FxHashMap<String, Vec<(String, f32)>>,
}

impl<'f> Adopting<'f> {
    pub(super) fn new(faces: &'f Faces) -> Self {
        Self {
            faces,
            shares: FxHashMap::default(),
        }
    }

    pub(super) fn keys(&mut self, key: MorphKey) -> impl Iterator<Item = MorphKey> + '_ {
        if !self.shares.contains_key(&key.morph) {
            let adopted = self.faces.adopt(&key.morph);
            let shares = match adopted.is_empty() {
                true => vec![(key.morph.clone(), 1.0)],
                false => adopted
                    .into_iter()
                    .map(|(morph, share)| (morph.to_owned(), share))
                    .collect(),
            };
            self.shares.insert(key.morph.clone(), shares);
        }
        self.shares[&key.morph]
            .iter()
            .map(move |(morph, share)| MorphKey {
                morph: morph.clone(),
                weight: key.weight * share,
                ..key.clone()
            })
    }
}
