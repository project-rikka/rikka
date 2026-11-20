use animate_body::Side;
use rikka_model_core::glam::Vec3;
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::hold::{Hold, HoldTarget, Holder, MAX_EASE};
use crate::holds::Holds;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum HandFile {
    Left,
    Right,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum HolderFile {
    Hand(HandFile),
    Pin { node: u32, at: [f32; 3] },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct TargetFile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    figure: Option<String>,
    at: [f32; 3],
    reach: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HoldFile {
    holder: HolderFile,
    cloth: TargetFile,
    from: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    until: Option<u32>,
    ease: u32,
}

impl HoldFile {
    #[must_use]
    pub fn from_holds(holds: &Holds) -> Vec<HoldFile> {
        holds.iter().map(HoldFile::from_hold).collect()
    }

    pub fn holds(files: &[HoldFile]) -> Result<Holds> {
        let mut holds = Vec::with_capacity(files.len());
        for (index, file) in files.iter().enumerate() {
            holds.push(file.hold(index)?);
        }
        Ok(Holds::from_list(holds))
    }

    fn from_hold(hold: &Hold) -> HoldFile {
        HoldFile {
            holder: match hold.holder {
                Holder::Hand(Side::Left) => HolderFile::Hand(HandFile::Left),
                Holder::Hand(Side::Right) => HolderFile::Hand(HandFile::Right),
                Holder::Pin { node, at } => HolderFile::Pin {
                    node,
                    at: at.to_array(),
                },
            },
            cloth: TargetFile {
                figure: hold.target.figure.clone(),
                at: hold.target.at.to_array(),
                reach: hold.target.reach,
            },
            from: hold.from,
            until: hold.until,
            ease: hold.ease,
        }
    }

    fn hold(&self, index: usize) -> Result<Hold> {
        let point = |at: [f32; 3]| {
            let point = Vec3::from_array(at);
            point
                .is_finite()
                .then_some(point)
                .ok_or(Error::NonFinitePoint(index))
        };
        let holder = match &self.holder {
            HolderFile::Hand(HandFile::Left) => Holder::Hand(Side::Left),
            HolderFile::Hand(HandFile::Right) => Holder::Hand(Side::Right),
            HolderFile::Pin { node, at } => Holder::Pin {
                node: *node,
                at: point(*at)?,
            },
        };
        if !(self.cloth.reach.is_finite() && self.cloth.reach > 0.0) {
            return Err(Error::NonPositiveReach(index));
        }
        if self.until.is_some_and(|until| until <= self.from) {
            return Err(Error::EndsBeforeStart(index));
        }
        if self.ease > MAX_EASE {
            return Err(Error::EaseTooLong(index));
        }
        Ok(Hold {
            holder,
            target: HoldTarget {
                figure: self.cloth.figure.clone(),
                at: point(self.cloth.at)?,
                reach: self.cloth.reach,
            },
            from: self.from,
            until: self.until,
            ease: self.ease,
        })
    }
}
