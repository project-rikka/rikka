use rikka_xdg::Group;

use crate::keys;

const DEFAULT_THRESHOLD: u32 = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Fixed,
    Scalable,
    Threshold,
}

#[derive(Clone, Debug)]
pub(crate) struct Directory {
    pub(crate) path: String,
    size: u32,
    scale: u32,
    kind: Kind,
    minimum: u32,
    maximum: u32,
    threshold: u32,
}

impl Directory {
    pub(crate) fn read(path: &str, group: &Group) -> Option<Directory> {
        let size = keys::number(group, "Size")?;
        Some(Directory {
            path: path.to_owned(),
            size,
            scale: keys::number(group, "Scale").unwrap_or(1).max(1),
            kind: match group.raw("Type") {
                Some("Fixed") => Kind::Fixed,
                Some("Scalable") => Kind::Scalable,
                _ => Kind::Threshold,
            },
            minimum: keys::number(group, "MinSize").unwrap_or(size),
            maximum: keys::number(group, "MaxSize").unwrap_or(size),
            threshold: keys::number(group, "Threshold").unwrap_or(DEFAULT_THRESHOLD),
        })
    }

    pub(crate) fn matches(&self, size: u32, scale: u32) -> bool {
        if self.scale != scale {
            return false;
        }
        match self.kind {
            Kind::Fixed => self.size == size,
            Kind::Scalable => self.minimum <= size && size <= self.maximum,
            Kind::Threshold => self.floor() <= size && size <= self.ceiling(),
        }
    }

    pub(crate) fn distance(&self, size: u32, scale: u32) -> u32 {
        let requested = size.saturating_mul(scale);
        let scaled = |side: u32| side.saturating_mul(self.scale);
        match self.kind {
            Kind::Fixed => scaled(self.size).abs_diff(requested),
            Kind::Scalable => clamp_distance(requested, scaled(self.minimum), scaled(self.maximum)),
            Kind::Threshold => {
                clamp_distance(requested, scaled(self.floor()), scaled(self.ceiling()))
            }
        }
    }

    fn floor(&self) -> u32 {
        self.size.saturating_sub(self.threshold)
    }

    fn ceiling(&self) -> u32 {
        self.size.saturating_add(self.threshold)
    }
}

fn clamp_distance(requested: u32, minimum: u32, maximum: u32) -> u32 {
    if requested < minimum {
        return minimum - requested;
    }
    if requested > maximum {
        return requested - maximum;
    }
    0
}
