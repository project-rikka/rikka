use crate::family::Family;
use crate::matching::StyleRequest;
use crate::style::{FontStyle, Weight, Width};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Query {
    families: Vec<Family>,
    weight: Weight,
    width: Width,
    style: FontStyle,
}

impl Query {
    pub fn new<F: Into<Family>>(families: impl IntoIterator<Item = F>) -> Self {
        Self {
            families: families.into_iter().map(Into::into).collect(),
            weight: Weight::NORMAL,
            width: Width::NORMAL,
            style: FontStyle::Normal,
        }
    }

    #[must_use]
    pub fn weight(mut self, weight: Weight) -> Self {
        self.weight = weight;
        self
    }

    #[must_use]
    pub fn width(mut self, width: Width) -> Self {
        self.width = width;
        self
    }

    #[must_use]
    pub fn style(mut self, style: FontStyle) -> Self {
        self.style = style;
        self
    }

    pub fn families(&self) -> &[Family] {
        &self.families
    }

    #[must_use]
    pub fn with_families<F: Into<Family>>(&self, families: impl IntoIterator<Item = F>) -> Self {
        Self {
            families: families.into_iter().map(Into::into).collect(),
            weight: self.weight,
            width: self.width,
            style: self.style,
        }
    }

    pub(crate) fn request(&self) -> StyleRequest {
        StyleRequest {
            weight: self.weight,
            width: self.width,
            style: self.style,
        }
    }
}
