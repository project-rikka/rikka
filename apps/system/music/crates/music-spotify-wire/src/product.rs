use crate::error::{Error, Result};

const TYPE: &str = "type";
const CATALOGUE: &str = "catalogue";
const FILTER_EXPLICIT: &str = "filter-explicit-content";
const PREMIUM: &str = "premium";
const ENABLED: &str = "1";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Product {
    pub account: Option<AccountType>,
    pub catalogue: Option<String>,
    pub filters_explicit: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AccountType {
    Premium,
    Other(String),
}

impl Product {
    pub fn parse(xml: &str) -> Result<Product> {
        let root = rikka_xml::parse_lenient(xml).ok_or(Error::Malformed("product info"))?;
        let mut product = Product {
            account: None,
            catalogue: None,
            filters_explicit: false,
        };
        for element in root.descendants() {
            let text = element.text();
            if text.is_empty() {
                continue;
            }
            match element.name() {
                TYPE => {
                    product.account = Some(match text.as_ref() {
                        PREMIUM => AccountType::Premium,
                        other => AccountType::Other(other.to_owned()),
                    });
                }
                CATALOGUE => product.catalogue = Some(text.into_owned()),
                FILTER_EXPLICIT => product.filters_explicit = text == ENABLED,
                _ => {}
            }
        }
        Ok(product)
    }
}
