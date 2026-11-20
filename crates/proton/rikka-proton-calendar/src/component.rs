use std::fmt;

use rikka_proton_contentline::{ContentLine, escape_text, read_lines, unescape_text};

use crate::error::{Error, Result};

const BEGIN: &str = "BEGIN";
const END: &str = "END";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Component {
    name: String,
    properties: Vec<ContentLine>,
    components: Vec<Component>,
}

impl Component {
    #[must_use]
    pub fn new(name: impl AsRef<str>) -> Component {
        Component {
            name: name.as_ref().to_ascii_uppercase(),
            properties: Vec::new(),
            components: Vec::new(),
        }
    }

    pub fn parse_all(text: &str) -> Result<Vec<Component>> {
        let mut finished = Vec::new();
        let mut open: Vec<Component> = Vec::new();

        for line in read_lines(text)? {
            if line.is(BEGIN) {
                open.push(Component::new(line.value().trim()));
                continue;
            }
            if line.is(END) {
                let closing = line.value().trim();
                let component = open
                    .pop()
                    .ok_or_else(|| Error::Unbalanced(closing.to_owned()))?;
                if !component.name.eq_ignore_ascii_case(closing) {
                    return Err(Error::Unbalanced(closing.to_owned()));
                }
                match open.last_mut() {
                    Some(parent) => parent.components.push(component),
                    None => finished.push(component),
                }
                continue;
            }
            if let Some(current) = open.last_mut() {
                current.properties.push(line);
            }
        }

        match open.pop() {
            Some(unclosed) => Err(Error::Unbalanced(unclosed.name)),
            None => Ok(finished),
        }
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn is(&self, name: &str) -> bool {
        self.name.eq_ignore_ascii_case(name)
    }

    #[must_use]
    pub fn properties(&self) -> &[ContentLine] {
        &self.properties
    }

    pub fn properties_mut(&mut self) -> &mut Vec<ContentLine> {
        &mut self.properties
    }

    #[must_use]
    pub fn components(&self) -> &[Component] {
        &self.components
    }

    pub fn components_mut(&mut self) -> &mut Vec<Component> {
        &mut self.components
    }

    pub fn property(&self, name: &str) -> Option<&ContentLine> {
        self.properties.iter().find(|line| line.is(name))
    }

    pub fn all<'a, 'n>(
        &'a self,
        name: &'n str,
    ) -> impl Iterator<Item = &'a ContentLine> + use<'a, 'n> {
        self.properties.iter().filter(move |line| line.is(name))
    }

    #[must_use]
    pub fn text(&self, name: &str) -> Option<String> {
        self.property(name).map(|line| unescape_text(line.value()))
    }

    pub fn children<'a, 'n>(
        &'a self,
        name: &'n str,
    ) -> impl Iterator<Item = &'a Component> + use<'a, 'n> {
        self.components
            .iter()
            .filter(move |component| component.is(name))
    }

    pub fn push(&mut self, line: ContentLine) {
        self.properties.push(line);
    }

    pub fn push_component(&mut self, component: Component) {
        self.components.push(component);
    }

    pub fn set(&mut self, line: ContentLine) {
        let name = line.name().to_owned();
        match self
            .properties
            .iter()
            .position(|current_property| current_property.is(&name))
        {
            Some(index) => {
                self.properties[index] = line;
                let mut found = false;
                self.properties.retain(|current_property| {
                    if !current_property.is(&name) {
                        return true;
                    }
                    let keep = !found;
                    found = true;
                    keep
                });
            }
            None => self.properties.push(line),
        }
    }

    pub fn set_text(&mut self, name: &str, text: &str) {
        self.set(ContentLine::new(name, escape_text(text)));
    }

    pub fn remove(&mut self, name: &str) {
        self.properties.retain(|line| !line.is(name));
    }

    pub fn retain_properties(&mut self, keep: impl FnMut(&ContentLine) -> bool) {
        self.properties.retain(keep);
    }

    pub fn remove_components(&mut self, name: &str) {
        self.components.retain(|component| !component.is(name));
    }
}

impl fmt::Display for Component {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", ContentLine::new(BEGIN, &self.name))?;
        for line in &self.properties {
            write!(formatter, "{line}")?;
        }
        for component in &self.components {
            write!(formatter, "{component}")?;
        }
        write!(formatter, "{}", ContentLine::new(END, &self.name))
    }
}
