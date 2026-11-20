use std::fmt;

use rikka_proton_contentline::{ContentLine, read_lines};

use crate::error::{Error, Result};
use crate::property::Property;
use crate::quoted_printable::join_soft_line_breaks;
use crate::structured::{PostalAddress, StructuredName};
use crate::version::Version;

pub(crate) const VCARD: &str = "VCARD";
const BEGIN: &str = "BEGIN";
const END: &str = "END";
const VERSION: &str = "VERSION";
const FN: &str = "FN";
const UID: &str = "UID";
const EMAIL: &str = "EMAIL";
const CATEGORIES: &str = "CATEGORIES";
const N: &str = "N";
const ADR: &str = "ADR";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VCard {
    version: Version,
    properties: Vec<Property>,
}

impl VCard {
    #[must_use]
    pub fn new() -> VCard {
        VCard::default()
    }

    #[must_use]
    pub fn with_version(version: Version) -> VCard {
        VCard {
            version,
            properties: Vec::new(),
        }
    }

    pub fn parse(text: &str) -> Result<VCard> {
        let mut cards = VCard::parse_all(text)?;
        match cards.len() {
            1 => Ok(cards.remove(0)),
            0 => Err(Error::NoVCard),
            found => Err(Error::SeveralVCards(found)),
        }
    }

    pub fn parse_all(text: &str) -> Result<Vec<VCard>> {
        let mut cards = Vec::new();
        let mut open: Option<VCard> = None;
        let mut nested = 0usize;

        for line in read_lines(&join_soft_line_breaks(text))? {
            let boundary =
                |name: &str| line.is(name) && line.value().trim().eq_ignore_ascii_case(VCARD);
            if boundary(BEGIN) {
                if open.is_some() {
                    nested += 1;
                } else {
                    open = Some(VCard::new());
                }
                continue;
            }
            if boundary(END) {
                if nested > 0 {
                    nested -= 1;
                    continue;
                }
                cards.push(open.take().ok_or(Error::UnbalancedVCard)?);
                continue;
            }
            let Some(card) = open.as_mut() else {
                continue;
            };
            if nested > 0 {
                continue;
            }
            if line.is(VERSION) {
                card.version = Version::from(line.value().trim());
                continue;
            }
            card.properties.push(Property::from_line(line));
        }

        if open.is_some() {
            return Err(Error::UnbalancedVCard);
        }

        Ok(cards)
    }

    #[must_use]
    pub fn version(&self) -> &Version {
        &self.version
    }

    pub fn set_version(&mut self, version: Version) {
        self.version = version;
    }

    #[must_use]
    pub fn properties(&self) -> &[Property] {
        &self.properties
    }

    pub fn properties_mut(&mut self) -> &mut Vec<Property> {
        &mut self.properties
    }

    pub fn properties_named<'a, 'n>(
        &'a self,
        name: &'n str,
    ) -> impl Iterator<Item = &'a Property> + use<'a, 'n> {
        self.properties
            .iter()
            .filter(move |property| property.is(name))
    }

    #[must_use]
    pub fn first(&self, name: &str) -> Option<&Property> {
        self.properties_named(name).next()
    }

    #[must_use]
    pub fn with(mut self, property: Property) -> VCard {
        self.push(property);
        self
    }

    pub fn push(&mut self, property: Property) {
        self.properties.push(property);
    }

    pub fn remove_named(&mut self, name: &str) {
        self.properties.retain(|property| !property.is(name));
    }

    pub fn retain(&mut self, keep: impl FnMut(&Property) -> bool) {
        self.properties.retain(keep);
    }

    #[must_use]
    pub fn formatted_name(&self) -> Option<String> {
        self.first(FN)
            .map(Property::text_value)
            .filter(|name| !name.trim().is_empty())
    }

    #[must_use]
    pub fn uid(&self) -> Option<String> {
        self.first(UID)
            .map(Property::text_value)
            .filter(|uid| !uid.trim().is_empty())
    }

    pub fn emails(&self) -> impl Iterator<Item = &Property> {
        self.properties_named(EMAIL)
    }

    #[must_use]
    pub fn email_addresses(&self) -> Vec<String> {
        self.emails()
            .map(|email| email.text_value().trim().to_owned())
            .filter(|email| !email.is_empty())
            .collect()
    }

    #[must_use]
    pub fn categories(&self) -> Vec<String> {
        self.properties_named(CATEGORIES)
            .flat_map(Property::list_value)
            .map(|category| category.trim().to_owned())
            .filter(|category| !category.is_empty())
            .collect()
    }

    #[must_use]
    pub fn structured_name(&self) -> Option<StructuredName> {
        self.first(N).map(StructuredName::from_property)
    }

    #[must_use]
    pub fn addresses(&self) -> Vec<PostalAddress> {
        self.properties_named(ADR)
            .map(PostalAddress::from_property)
            .collect()
    }

    #[must_use]
    pub fn groups(&self) -> Vec<String> {
        let mut groups: Vec<String> = Vec::new();
        for group in self.properties.iter().filter_map(Property::group) {
            if !groups
                .iter()
                .any(|group_name| group_name.eq_ignore_ascii_case(group))
            {
                groups.push(group.to_owned());
            }
        }
        groups
    }

    #[must_use]
    pub fn email_group(&self, email: &str) -> Option<&str> {
        let target = email.trim();
        self.emails()
            .find(|property| property.text_value().trim().eq_ignore_ascii_case(target))
            .and_then(Property::group)
    }

    #[must_use]
    pub fn unused_group(&self) -> String {
        let groups = self.groups();
        free_group_name(|candidate| {
            groups
                .iter()
                .any(|group| group.eq_ignore_ascii_case(candidate))
        })
    }

    #[must_use]
    pub fn to_card_text(&self) -> String {
        let mut output = self.to_string();
        output.truncate(output.trim_end_matches("\r\n").len());
        output
    }
}

impl fmt::Display for VCard {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", ContentLine::new(BEGIN, VCARD))?;
        write!(
            formatter,
            "{}",
            ContentLine::new(VERSION, self.version.as_str())
        )?;
        for property in &self.properties {
            write!(formatter, "{}", property.line())?;
        }
        write!(formatter, "{}", ContentLine::new(END, VCARD))
    }
}

#[must_use]
pub fn write_vcards(cards: &[VCard]) -> String {
    cards.iter().map(VCard::to_string).collect()
}

pub(crate) fn free_group_name(is_taken: impl Fn(&str) -> bool) -> String {
    let mut index = 1usize;
    loop {
        let candidate = format!("item{index}");
        if !is_taken(&candidate) {
            return candidate;
        }
        index += 1;
    }
}
