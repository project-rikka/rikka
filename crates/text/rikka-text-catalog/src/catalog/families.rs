use std::collections::HashMap;

use crate::face::{Face, FaceId};
use crate::family::fold;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct FamilyId(u32);

#[derive(Debug, Default)]
pub(crate) struct FamilyTable {
    names: Vec<Box<str>>,
    members: Vec<Vec<FaceId>>,
    by_name: HashMap<String, FamilyId>,
}

impl FamilyTable {
    pub(crate) fn build(faces: &[Face]) -> Self {
        let mut table = Self::default();
        for (index, face) in faces.iter().enumerate() {
            for (name, folded) in face.record.families.iter().zip(&face.folded_families) {
                let next = FamilyId(table.names.len() as u32);
                let id = *table.by_name.entry(folded.clone()).or_insert(next);
                if id == next {
                    table.names.push(name.clone());
                    table.members.push(Vec::new());
                }
                table.members[id.0 as usize].push(FaceId(index as u32));
            }
        }
        table
    }

    pub(crate) fn find(&self, name: &str) -> Option<FamilyId> {
        self.by_name.get(&fold(name)).copied()
    }

    pub(crate) fn primary_of(&self, face: &Face) -> Option<FamilyId> {
        self.by_name.get(&face.folded_families[0]).copied()
    }

    pub(crate) fn faces(&self, family: FamilyId) -> &[FaceId] {
        &self.members[family.0 as usize]
    }

    pub(crate) fn names(&self) -> impl ExactSizeIterator<Item = &str> {
        self.names.iter().map(|name| &**name)
    }
}
