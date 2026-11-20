use crate::catalog::families::{FamilyId, FamilyTable};
use crate::config::Aliases;
use crate::family::Family;
use crate::system::preferred_families;

const MAX_DEFAULT_ROUNDS: usize = 4;

const GENERICS: [Family; 8] = [
    Family::SansSerif,
    Family::Serif,
    Family::Monospace,
    Family::Emoji,
    Family::SystemUi,
    Family::Cursive,
    Family::Fantasy,
    Family::Math,
];

#[derive(Debug, Default)]
pub(crate) struct Generics {
    chains: Vec<(Family, Box<[FamilyId]>)>,
}

impl Generics {
    pub(crate) fn build(table: &FamilyTable, aliases: &Aliases) -> Self {
        let mut generics = Self::default();
        for generic in GENERICS {
            let mut resolver = Resolver::new(table, aliases, &generics);
            for name in preferred_families(&generic) {
                resolver.push_name(name, Strength::Weak);
            }
            if let Some(alias) = aliases.get(generic.name()) {
                for name in alias.prefer.iter().chain(&alias.accept) {
                    resolver.push_name(name, Strength::Weak);
                }
                resolver.defaults.extend(alias.default.iter().cloned());
            }
            resolver.expand_defaults();
            if generic != Family::SansSerif {
                resolver.push_generic(&Family::SansSerif, Strength::Weak);
            }
            let chain = resolver.finish().families;
            generics.chains.push((generic, chain.into()));
        }
        generics
    }

    pub(crate) fn chain(&self, generic: &Family) -> &[FamilyId] {
        self.chains
            .iter()
            .find(|(family, _)| family == generic)
            .map_or(&[], |(_, chain)| chain)
    }
}

pub(crate) struct Resolution {
    pub(crate) families: Vec<FamilyId>,
    pub(crate) fallback: Vec<FamilyId>,
}

pub(crate) fn resolve(
    families: &[Family],
    table: &FamilyTable,
    aliases: &Aliases,
    generics: &Generics,
) -> Resolution {
    let mut resolver = Resolver::new(table, aliases, generics);
    for family in families {
        match family {
            Family::Named(name) => resolver.push_named(name, Strength::Strong),
            generic => resolver.push_generic(generic, Strength::Strong),
        }
    }
    resolver.expand_defaults();
    resolver.finish()
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Strength {
    Strong,
    Weak,
}

struct Resolver<'a> {
    table: &'a FamilyTable,
    aliases: &'a Aliases,
    generics: &'a Generics,
    families: Vec<FamilyId>,
    fallback: Vec<FamilyId>,
    defaults: Vec<String>,
}

impl<'a> Resolver<'a> {
    fn new(table: &'a FamilyTable, aliases: &'a Aliases, generics: &'a Generics) -> Self {
        Self {
            table,
            aliases,
            generics,
            families: Vec::new(),
            fallback: Vec::new(),
            defaults: Vec::new(),
        }
    }

    fn push_named(&mut self, name: &str, strength: Strength) {
        let Some(alias) = self.aliases.get(name) else {
            self.push_family(name, strength);
            return;
        };
        for preferred in &alias.prefer {
            self.push_name(preferred, strength);
        }
        self.push_family(name, strength);
        for accepted in &alias.accept {
            self.push_name(accepted, strength);
        }
        self.defaults.extend(alias.default.iter().cloned());
    }

    fn push_name(&mut self, name: &str, strength: Strength) {
        match Family::parse(name) {
            Family::Named(_) => self.push_family(name, strength),
            generic => self.push_generic(&generic, strength),
        }
    }

    fn push_family(&mut self, name: &str, strength: Strength) {
        if let Some(id) = self.table.find(name) {
            self.families.push(id);
            if strength == Strength::Strong {
                self.fallback.push(id);
            }
        }
    }

    fn push_generic(&mut self, generic: &Family, strength: Strength) {
        let chain = self.generics.chain(generic);
        self.families.extend_from_slice(chain);
        if strength == Strength::Strong {
            self.fallback.extend(chain.first());
        }
    }

    fn expand_defaults(&mut self) {
        for _ in 0..MAX_DEFAULT_ROUNDS {
            if self.defaults.is_empty() {
                break;
            }
            for name in std::mem::take(&mut self.defaults) {
                match Family::parse(&name) {
                    Family::Named(named) => self.push_named(&named, Strength::Weak),
                    generic => self.push_generic(&generic, Strength::Weak),
                }
            }
        }
    }

    fn finish(self) -> Resolution {
        Resolution {
            families: deduplicated(self.families),
            fallback: deduplicated(self.fallback),
        }
    }
}

fn deduplicated(ids: Vec<FamilyId>) -> Vec<FamilyId> {
    let mut unique = Vec::with_capacity(ids.len());
    for id in ids {
        if !unique.contains(&id) {
            unique.push(id);
        }
    }
    unique
}
