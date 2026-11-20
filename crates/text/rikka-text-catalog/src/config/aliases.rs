use std::collections::HashMap;

use rikka_xml::Element;

use crate::family::fold;

#[derive(Debug, Default)]
pub(crate) struct Aliases {
    entries: HashMap<String, Alias>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Alias {
    pub(crate) prefer: Vec<String>,
    pub(crate) accept: Vec<String>,
    pub(crate) default: Vec<String>,
}

impl Aliases {
    pub(crate) fn add(&mut self, element: &Element<'_>) {
        let tested = element
            .children_named("family")
            .map(Element::text)
            .filter(|family| !family.is_empty());
        let prefer = families_in(element, "prefer");
        let accept = families_in(element, "accept");
        let default = families_in(element, "default");
        for family in tested {
            let alias = self.entries.entry(fold(&family)).or_default();
            alias.prefer.extend(prefer.iter().cloned());
            alias.accept.splice(0..0, accept.iter().cloned());
            alias.default.extend(default.iter().cloned());
        }
    }

    #[cfg(windows)]
    pub(crate) fn insert(&mut self, family: &str, alias: Alias) {
        self.entries.insert(fold(family), alias);
    }

    pub(crate) fn get(&self, family: &str) -> Option<&Alias> {
        self.entries.get(&fold(family))
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = (&str, &Alias)> {
        self.entries
            .iter()
            .map(|(family, alias)| (family.as_str(), alias))
    }
}

fn families_in(element: &Element<'_>, list: &str) -> Vec<String> {
    element
        .children_named(list)
        .flat_map(|list| list.children_named("family").map(Element::text))
        .filter(|family| !family.is_empty())
        .map(|family| family.into_owned())
        .collect()
}
