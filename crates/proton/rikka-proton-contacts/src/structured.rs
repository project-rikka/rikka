use crate::property::Property;

const N: &str = "N";
const ADR: &str = "ADR";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StructuredName {
    pub family: Vec<String>,
    pub given: Vec<String>,
    pub additional: Vec<String>,
    pub prefixes: Vec<String>,
    pub suffixes: Vec<String>,
}

impl StructuredName {
    #[must_use]
    pub fn from_property(property: &Property) -> StructuredName {
        let mut components = property.components().into_iter().map(drop_empty_component);
        StructuredName {
            family: components.next().unwrap_or_default(),
            given: components.next().unwrap_or_default(),
            additional: components.next().unwrap_or_default(),
            prefixes: components.next().unwrap_or_default(),
            suffixes: components.next().unwrap_or_default(),
        }
    }

    #[must_use]
    pub fn to_property(&self) -> Property {
        Property::structured(
            N,
            &[
                self.family.clone(),
                self.given.clone(),
                self.additional.clone(),
                self.prefixes.clone(),
                self.suffixes.clone(),
            ],
        )
    }

    #[must_use]
    pub fn display_name(&self) -> String {
        [self.given.first(), self.family.first()]
            .into_iter()
            .flatten()
            .map(|part| part.trim())
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join(" ")
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PostalAddress {
    pub post_office_box: String,
    pub extended: String,
    pub street: String,
    pub locality: String,
    pub region: String,
    pub postal_code: String,
    pub country: String,
    pub types: Vec<String>,
}

impl PostalAddress {
    #[must_use]
    pub fn from_property(property: &Property) -> PostalAddress {
        let mut components = property
            .components()
            .into_iter()
            .map(|component| component.join(", "));
        PostalAddress {
            post_office_box: components.next().unwrap_or_default(),
            extended: components.next().unwrap_or_default(),
            street: components.next().unwrap_or_default(),
            locality: components.next().unwrap_or_default(),
            region: components.next().unwrap_or_default(),
            postal_code: components.next().unwrap_or_default(),
            country: components.next().unwrap_or_default(),
            types: property.types(),
        }
    }

    #[must_use]
    pub fn to_property(&self) -> Property {
        let property = Property::structured(
            ADR,
            &[
                vec![self.post_office_box.as_str()],
                vec![self.extended.as_str()],
                vec![self.street.as_str()],
                vec![self.locality.as_str()],
                vec![self.region.as_str()],
                vec![self.postal_code.as_str()],
                vec![self.country.as_str()],
            ],
        );
        match self.types.is_empty() {
            true => property,
            false => property.with_types(self.types.iter().cloned()),
        }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        [
            &self.post_office_box,
            &self.extended,
            &self.street,
            &self.locality,
            &self.region,
            &self.postal_code,
            &self.country,
        ]
        .iter()
        .all(|part| part.trim().is_empty())
    }
}

fn drop_empty_component(component: Vec<String>) -> Vec<String> {
    match component.as_slice() {
        [only] if only.is_empty() => Vec::new(),
        _ => component,
    }
}
