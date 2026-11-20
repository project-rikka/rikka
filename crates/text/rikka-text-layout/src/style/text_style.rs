use rikka_text_catalog::{Family, FontStyle, Query, Weight, Width};

use crate::style::Caps;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[must_use]
pub struct TextStyle {
    query: Query,
    pub(crate) caps: Caps,
}

impl TextStyle {
    pub fn new<F: Into<Family>>(families: impl IntoIterator<Item = F>) -> Self {
        Self {
            query: Query::new(families),
            caps: Caps::Normal,
        }
    }

    pub fn weight(mut self, weight: Weight) -> Self {
        self.query = self.query.weight(weight);
        self
    }

    pub fn width(mut self, width: Width) -> Self {
        self.query = self.query.width(width);
        self
    }

    pub fn style(mut self, style: FontStyle) -> Self {
        self.query = self.query.style(style);
        self
    }

    pub fn caps(mut self, caps: Caps) -> Self {
        self.caps = caps;
        self
    }

    pub(crate) fn query(&self) -> &Query {
        &self.query
    }

    pub(crate) fn overridden(
        &self,
        family: Option<&Family>,
        weight: Option<Weight>,
        style: Option<FontStyle>,
    ) -> Query {
        let mut query = match family {
            Some(family) => self.query.with_families([family.clone()]),
            None => self.query.clone(),
        };
        if let Some(weight) = weight {
            query = query.weight(weight);
        }
        if let Some(style) = style {
            query = query.style(style);
        }
        query
    }
}

impl Default for TextStyle {
    fn default() -> Self {
        Self::new([Family::SansSerif])
    }
}
