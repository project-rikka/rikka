use crate::layout::{EntryPoint, Parameter, TypeLayout};

#[derive(Clone, Debug)]
#[must_use]
pub struct Source<'a> {
    pub(crate) name: &'a str,
    pub(crate) text: &'a str,
    pub(crate) entries: Vec<&'a str>,
    pub(crate) types: Vec<&'a str>,
    pub(crate) conformances: Vec<(&'a str, &'a str)>,
    pub(crate) flatten: bool,
}

impl<'a> Source<'a> {
    pub fn new(name: &'a str, text: &'a str) -> Self {
        Self {
            name,
            text,
            entries: Vec::new(),
            types: Vec::new(),
            conformances: Vec::new(),
            flatten: false,
        }
    }

    pub fn entry(mut self, name: &'a str) -> Self {
        self.entries.push(name);
        self
    }

    pub fn layout_of(mut self, type_name: &'a str) -> Self {
        self.types.push(type_name);
        self
    }

    pub fn conformance(mut self, type_name: &'a str, interface: &'a str) -> Self {
        self.conformances.push((type_name, interface));
        self
    }

    pub fn flatten(mut self) -> Self {
        self.flatten = true;
        self
    }
}

#[derive(Clone, Debug)]
pub struct Program {
    entry_points: Vec<EntryPoint>,
    parameters: Vec<Parameter>,
    types: Vec<(String, TypeLayout)>,
    conformances: Vec<(String, String, bool)>,
}

impl Program {
    pub(crate) fn new(
        entry_points: Vec<EntryPoint>,
        parameters: Vec<Parameter>,
        types: Vec<(String, TypeLayout)>,
        conformances: Vec<(String, String, bool)>,
    ) -> Self {
        Self {
            entry_points,
            parameters,
            types,
            conformances,
        }
    }

    pub fn conforms(&self, type_name: &str, interface: &str) -> Option<bool> {
        self.conformances
            .iter()
            .find(|(ty, known, _)| ty == type_name && known == interface)
            .map(|(_, _, conforms)| *conforms)
    }

    pub fn entry_points(&self) -> &[EntryPoint] {
        &self.entry_points
    }

    pub fn entry_point(&self, name: &str) -> Option<&EntryPoint> {
        self.entry_points.iter().find(|entry| entry.name == name)
    }

    pub fn parameters(&self) -> &[Parameter] {
        &self.parameters
    }

    pub fn parameter(&self, name: &str) -> Option<&Parameter> {
        self.parameters
            .iter()
            .find(|parameter| parameter.name == name)
    }

    pub fn type_layout(&self, name: &str) -> Option<&TypeLayout> {
        self.types
            .iter()
            .find(|(declared, _)| declared == name)
            .map(|(_, layout)| layout)
    }
}
