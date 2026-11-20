use crate::property::Property;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StatusArea {
    pub properties: Vec<Property>,
    pub input_methods: Vec<Property>,
    pub menu: Option<Menu>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Menu {
    pub key: String,
    pub entries: Vec<Property>,
}

impl StatusArea {
    #[must_use]
    pub fn current(&self) -> Option<&Property> {
        self.properties
            .iter()
            .find(|property| property.is_current_input_method())
    }

    pub fn actions(&self) -> impl Iterator<Item = &Property> {
        self.properties
            .iter()
            .filter(|property| !property.is_current_input_method())
    }

    #[must_use]
    pub fn menu(&self, key: &str) -> Option<&[Property]> {
        self.menu
            .as_ref()
            .filter(|menu| menu.key == key)
            .map(|menu| menu.entries.as_slice())
    }
}
