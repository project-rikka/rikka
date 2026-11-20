use std::borrow::Cow;

use rikka_types::Color;

use crate::parse::colors::color;
use crate::parse::dom::{Dom, NodeId};
use crate::parse::values::{Length, Unit, length, number, unit_interval};

const INHERITED: [&str; 34] = [
    "clip-rule",
    "color",
    "color-interpolation",
    "color-interpolation-filters",
    "dominant-baseline",
    "fill",
    "fill-opacity",
    "fill-rule",
    "font-family",
    "font-size",
    "font-stretch",
    "font-style",
    "font-variant",
    "font-variant-caps",
    "font-weight",
    "image-rendering",
    "letter-spacing",
    "marker-end",
    "marker-mid",
    "marker-start",
    "paint-order",
    "shape-rendering",
    "stroke",
    "stroke-dasharray",
    "stroke-dashoffset",
    "stroke-linecap",
    "stroke-linejoin",
    "stroke-miterlimit",
    "stroke-opacity",
    "stroke-width",
    "text-anchor",
    "text-rendering",
    "white-space",
    "word-spacing",
];

const DEFAULT_FONT_SIZE: f32 = 16.0;
const MAX_SUBSTITUTIONS: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Viewport {
    pub(crate) width: f32,
    pub(crate) height: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Axis {
    Horizontal,
    Vertical,
    Diagonal,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Context<'c> {
    pub(crate) dom: &'c Dom,
    pub(crate) chain: &'c [NodeId],
    pub(crate) viewport: Viewport,
}

impl<'c> Context<'c> {
    fn raw(&self, name: &str) -> Option<&'c str> {
        let inherited = INHERITED.contains(&name) || name == "visibility" || name.starts_with("--");
        let (node, ancestors) = self.chain.split_last()?;
        let own = self.dom.attribute(*node, name);
        match own {
            Some("inherit") => {}
            Some(value) => return Some(value),
            None if !inherited => return None,
            None => {}
        }
        ancestors
            .iter()
            .rev()
            .filter_map(|ancestor| self.dom.attribute(*ancestor, name))
            .find(|value| *value != "inherit")
    }

    pub(crate) fn property(&self, name: &str) -> Option<Cow<'c, str>> {
        let mut value = Cow::Borrowed(self.raw(name)?);
        for _ in 0..MAX_SUBSTITUTIONS {
            let Some(start) = value.find("var(") else {
                break;
            };
            let Some(length) = closing(&value[start + 4..]) else {
                break;
            };
            let inner = &value[start + 4..start + 4 + length];
            let (variable, fallback) = match inner.split_once(',') {
                Some((variable, fallback)) => (variable.trim(), fallback.trim()),
                None => (inner.trim(), ""),
            };
            let chosen = self.raw(variable).unwrap_or(fallback);
            let replaced = format!(
                "{}{}{}",
                &value[..start],
                chosen,
                &value[start + 4 + length + 1..]
            );
            value = Cow::Owned(replaced);
        }
        let trimmed = value.trim();
        match trimmed.is_empty() {
            true => None,
            false if trimmed.len() == value.len() => Some(value),
            false => Some(Cow::Owned(trimmed.to_owned())),
        }
    }

    pub(crate) fn attribute(&self, name: &str) -> Option<&'c str> {
        let node = self.chain.last()?;
        self.dom.attribute(*node, name)
    }

    pub(crate) fn is(&self, name: &str, expected: &str) -> bool {
        self.property(name).is_some_and(|value| value == expected)
    }

    pub(crate) fn current_color(&self) -> Color {
        self.property("color")
            .and_then(|value| color(&value))
            .unwrap_or(Color::rgba(0.0, 0.0, 0.0, 1.0))
    }

    pub(crate) fn color(&self, name: &str) -> Option<Color> {
        let value = self.property(name)?;
        match value.as_ref() {
            "currentColor" | "currentcolor" => Some(self.current_color()),
            other => color(other),
        }
    }

    pub(crate) fn unit(&self, name: &str) -> Option<f32> {
        unit_interval(&self.property(name)?)
    }

    pub(crate) fn number(&self, name: &str) -> Option<f32> {
        number(&self.property(name)?)
    }

    pub(crate) fn ancestry(&self, name: &str) -> impl Iterator<Item = &'c str> {
        let dom = self.dom;
        self.chain
            .iter()
            .filter_map(move |node| dom.attribute(*node, name))
    }

    pub(crate) fn font_size(&self) -> f32 {
        let mut size = DEFAULT_FONT_SIZE;
        for value in self.ancestry("font-size") {
            size = match value.trim() {
                "xx-small" => 9.0,
                "x-small" => 10.0,
                "small" => 13.0,
                "medium" => DEFAULT_FONT_SIZE,
                "large" => 18.0,
                "x-large" => 24.0,
                "xx-large" => 32.0,
                "larger" => size * 1.2,
                "smaller" => size / 1.2,
                other => match length(other) {
                    Some(Length {
                        value,
                        unit: Unit::User,
                    }) => value,
                    Some(Length {
                        value,
                        unit: Unit::Em,
                    }) => value * size,
                    Some(Length {
                        value,
                        unit: Unit::Ex,
                    }) => value * size * 0.5,
                    Some(Length {
                        value,
                        unit: Unit::Percent,
                    }) => value * size / 100.0,
                    None => size,
                },
            }
            .max(0.0);
        }
        size
    }

    pub(crate) fn resolve(&self, length: Length, axis: Axis) -> f32 {
        match length.unit {
            Unit::User => length.value,
            Unit::Em => length.value * self.font_size(),
            Unit::Ex => length.value * self.font_size() * 0.5,
            Unit::Percent => {
                let whole = match axis {
                    Axis::Horizontal => self.viewport.width,
                    Axis::Vertical => self.viewport.height,
                    Axis::Diagonal => {
                        (self.viewport.width.powi(2) + self.viewport.height.powi(2)).sqrt()
                            / std::f32::consts::SQRT_2
                    }
                };
                whole * length.value / 100.0
            }
        }
    }

    pub(crate) fn length(&self, name: &str, axis: Axis) -> Option<f32> {
        let length = length(&self.property(name)?)?;
        Some(self.resolve(length, axis))
    }

    pub(crate) fn coordinate(&self, name: &str, axis: Axis) -> Option<f32> {
        let length = length(self.attribute(name)?)?;
        Some(self.resolve(length, axis))
    }
}

fn closing(text: &str) -> Option<usize> {
    let mut depth = 0usize;
    for (at, byte) in text.bytes().enumerate() {
        match byte {
            b'(' => depth += 1,
            b')' if depth == 0 => return Some(at),
            b')' => depth -= 1,
            _ => {}
        }
    }
    None
}
