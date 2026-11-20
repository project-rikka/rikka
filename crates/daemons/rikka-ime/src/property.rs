use crate::names;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Property {
    pub key: String,
    pub label: String,
    pub icon: String,
    pub tip: String,
    pub hint: String,
}

const LABEL: &str = "label=";

const MENU: &str = "menu";

impl Property {
    #[must_use]
    pub fn parse(property: &str) -> Option<Property> {
        let mut fields = property.splitn(5, ':');
        Some(Property {
            key: fields.next()?.to_owned(),
            label: fields.next()?.to_owned(),
            icon: fields.next()?.to_owned(),
            tip: fields.next()?.to_owned(),
            hint: fields.next().unwrap_or_default().to_owned(),
        })
    }

    #[must_use]
    pub fn badge(&self) -> Option<&str> {
        self.hints()
            .find_map(|word| word.strip_prefix(LABEL))
            .filter(|badge| !badge.is_empty())
    }

    #[must_use]
    pub fn opens_menu(&self) -> bool {
        self.hints().any(|word| word == MENU)
    }

    #[must_use]
    pub fn input_method(&self) -> Option<&str> {
        self.key
            .strip_prefix(names::INPUT_METHOD_ENTRY)
            .filter(|name| !name.is_empty())
    }

    #[must_use]
    pub fn is_current_input_method(&self) -> bool {
        self.key == names::CURRENT_INPUT_METHOD
    }

    fn hints(&self) -> impl Iterator<Item = &str> {
        self.hint.split(',')
    }
}
