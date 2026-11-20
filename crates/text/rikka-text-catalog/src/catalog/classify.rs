use std::collections::HashMap;

use crate::config::Aliases;
use crate::face::{FaceClass, FaceRecord};
use crate::family::{Family, fold};
use crate::system::preferred_families;

const CLASSES: [(Family, FaceClass); 3] = [
    (Family::SansSerif, FaceClass::Sans),
    (Family::Serif, FaceClass::Serif),
    (Family::Monospace, FaceClass::Mono),
];

pub(crate) struct Classifier {
    declared: HashMap<String, FaceClass>,
    preferred: HashMap<String, FaceClass>,
}

impl Classifier {
    pub(crate) fn new(aliases: &Aliases) -> Self {
        let declared = aliases
            .iter()
            .filter_map(|(family, alias)| {
                let class = alias
                    .default
                    .iter()
                    .find_map(|default| class_of(&Family::parse(default)))?;
                Some((family.to_owned(), class))
            })
            .collect();
        let mut preferred = HashMap::new();
        for (generic, class) in &CLASSES {
            let listed = aliases
                .get(generic.name())
                .map(|alias| alias.prefer.as_slice())
                .unwrap_or_default();
            let names = preferred_families(generic)
                .iter()
                .copied()
                .chain(listed.iter().map(String::as_str));
            for name in names {
                preferred.entry(fold(name)).or_insert(*class);
            }
        }
        Self {
            declared,
            preferred,
        }
    }

    pub(crate) fn classify(&self, record: &FaceRecord, folded: &[String]) -> FaceClass {
        if record.monospace {
            return FaceClass::Mono;
        }
        let lookup = |table: &HashMap<String, FaceClass>| {
            folded.iter().find_map(|family| table.get(family).copied())
        };
        if let Some(class) = lookup(&self.declared).or_else(|| lookup(&self.preferred)) {
            return class;
        }
        let primary = folded[0].as_str();
        if primary.contains("mono") {
            FaceClass::Mono
        } else if primary.contains("sans") {
            FaceClass::Sans
        } else if primary.contains("serif") {
            FaceClass::Serif
        } else {
            FaceClass::Other
        }
    }
}

fn class_of(family: &Family) -> Option<FaceClass> {
    CLASSES
        .iter()
        .find(|(generic, _)| generic == family)
        .map(|&(_, class)| class)
}
