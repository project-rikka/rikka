use std::collections::BTreeMap;

use rikka_model_core::glam::Vec3;
use serde::{Deserialize, Serialize};

use crate::Preset;
use crate::anchor::{Anchor, Spot};
use crate::error::{Error, Result};
use crate::formula::Formula;
use crate::pour::Pour;
use crate::property::Property;
use crate::shape::Shape;
use crate::value::EmitterValue;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub(super) enum ValueFile {
    Number(f32),
    Three([f32; 3]),
}

impl ValueFile {
    pub(super) fn from_value(value: EmitterValue) -> Self {
        match value {
            EmitterValue::Number(number) => Self::Number(number),
            EmitterValue::Three(three) => Self::Three(three),
        }
    }

    pub(super) fn value(self) -> EmitterValue {
        match self {
            Self::Number(number) => EmitterValue::Number(number),
            Self::Three(three) => EmitterValue::Three(three),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(super) struct BoneFile {
    figure: String,
    bone: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(super) struct SpotFile {
    offset: [f32; 3],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    bone: Option<BoneFile>,
}

impl SpotFile {
    pub(super) fn from_spot(spot: &Spot) -> Self {
        Self {
            offset: spot.offset.to_array(),
            bone: match &spot.anchor {
                Anchor::World => None,
                Anchor::Bone { figure, bone } => Some(BoneFile {
                    figure: figure.clone(),
                    bone: bone.clone(),
                }),
            },
        }
    }

    pub(super) fn spot(&self) -> Spot {
        Spot {
            anchor: self.bone.as_ref().map_or(Anchor::World, |bone| {
                Anchor::bone(bone.figure.clone(), bone.bone.clone())
            }),
            offset: Vec3::from_array(self.offset),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(super) enum ShapeFile {
    Stream,
    Shower,
    Drip,
    Blob { size: f32 },
    Surface { figure: String, materials: Vec<u32> },
}

impl ShapeFile {
    pub(super) fn from_shape(shape: &Shape) -> Self {
        match shape {
            Shape::Stream => Self::Stream,
            Shape::Shower => Self::Shower,
            Shape::Drip => Self::Drip,
            Shape::Blob { size } => Self::Blob { size: *size },
            Shape::Surface { figure, materials } => Self::Surface {
                figure: figure.clone(),
                materials: materials.clone(),
            },
        }
    }

    pub(super) fn shape(&self) -> Result<Shape> {
        Ok(match self {
            Self::Stream => Shape::Stream,
            Self::Shower => Shape::Shower,
            Self::Drip => Shape::Drip,
            Self::Blob { size } if Shape::BLOB_SIZES.contains(size) => Shape::Blob { size: *size },
            Self::Blob { .. } => {
                return Err(Error::OutOfRange {
                    name: "blob size".into(),
                });
            }
            Self::Surface { figure, materials } => {
                Shape::surface(figure.clone(), materials.iter().copied())
            }
        })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(super) struct FormulaFile {
    preset: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    overrides: BTreeMap<String, ValueFile>,
}

impl FormulaFile {
    pub(super) fn from_formula(formula: &Formula) -> Self {
        Self {
            preset: formula.preset().id().to_owned(),
            overrides: formula
                .overrides()
                .map(|(property, value)| (property.id().to_owned(), ValueFile::from_value(value)))
                .collect(),
        }
    }

    pub(super) fn formula(&self) -> Result<Formula> {
        let preset =
            Preset::named(&self.preset).ok_or_else(|| Error::UnknownPreset(self.preset.clone()))?;
        let mut formula = Formula::from_preset(preset);
        for (id, value) in &self.overrides {
            let property = Property::named(id).ok_or_else(|| Error::UnknownProperty(id.clone()))?;
            let value = value.value();
            if !property.kind().fits(value) {
                return Err(Error::OutOfRange { name: id.clone() });
            }
            formula.set(property, value);
        }
        Ok(formula)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub(super) struct PourFile {
    start: u32,
    length: u32,
}

impl PourFile {
    pub(super) fn from_pour(pour: Pour) -> Self {
        Self {
            start: pour.start(),
            length: pour.length(),
        }
    }

    pub(super) fn pour(self) -> Result<Pour> {
        Pour::checked(self.start, self.length).ok_or_else(|| Error::OutOfRange {
            name: "pour".into(),
        })
    }
}
