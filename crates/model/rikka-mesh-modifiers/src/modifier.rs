use rikka_file_format::{Codec, DecodeError, Reader, Writer};

use crate::codec::{record, tagged};
use crate::kinds::armature::Armature;
use crate::kinds::array::Array;
use crate::kinds::bevel::Bevel;
use crate::kinds::boolean::Boolean;
use crate::kinds::data_transfer::DataTransfer;
use crate::kinds::decimate::Decimate;
use crate::kinds::displace::Displace;
use crate::kinds::edge_split::EdgeSplit;
use crate::kinds::mirror::Mirror;
use crate::kinds::shrinkwrap::Shrinkwrap;
use crate::kinds::smooth_by_angle::SmoothByAngle;
use crate::kinds::solidify::Solidify;
use crate::kinds::subdivision::Subdivision;
use crate::kinds::triangulate::Triangulate;
use crate::kinds::weld::Weld;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Purpose {
    #[default]
    Viewport,
    Render,
    EditMode,
    EditCage,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ModifierKind {
    Mirror(Mirror),
    Subdivision(Subdivision),
    Solidify(Solidify),
    Array(Array),
    Weld(Weld),
    Triangulate(Triangulate),
    EdgeSplit(EdgeSplit),
    Displace(Displace),
    Shrinkwrap(Shrinkwrap),
    DataTransfer(DataTransfer),
    SmoothByAngle(SmoothByAngle),
    Armature(Armature),
    Bevel(Bevel),
    Boolean(Boolean),
    Decimate(Decimate),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ModifierType {
    Mirror,
    Subdivision,
    Solidify,
    Array,
    Weld,
    Triangulate,
    EdgeSplit,
    Displace,
    Shrinkwrap,
    DataTransfer,
    SmoothByAngle,
    Armature,
    Bevel,
    Boolean,
    Decimate,
}

impl ModifierType {
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            ModifierType::Mirror => "Mirror",
            ModifierType::Subdivision => "Subdivision",
            ModifierType::Solidify => "Solidify",
            ModifierType::Array => "Array",
            ModifierType::Weld => "Weld",
            ModifierType::Triangulate => "Triangulate",
            ModifierType::EdgeSplit => "EdgeSplit",
            ModifierType::Displace => "Displace",
            ModifierType::Shrinkwrap => "Shrinkwrap",
            ModifierType::DataTransfer => "DataTransfer",
            ModifierType::SmoothByAngle => "Smooth by Angle",
            ModifierType::Armature => "Armature",
            ModifierType::Bevel => "Bevel",
            ModifierType::Boolean => "Boolean",
            ModifierType::Decimate => "Decimate",
        }
    }

    #[must_use]
    pub fn supports_edit_mode(self) -> bool {
        !matches!(self, ModifierType::Decimate)
    }

    #[must_use]
    pub fn enabled_in_edit_mode(self) -> bool {
        !matches!(
            self,
            ModifierType::Displace
                | ModifierType::DataTransfer
                | ModifierType::Armature
                | ModifierType::Boolean
                | ModifierType::Decimate
        )
    }

    #[must_use]
    pub fn supports_mapping(self) -> bool {
        matches!(
            self,
            ModifierType::Mirror
                | ModifierType::Subdivision
                | ModifierType::Solidify
                | ModifierType::Array
                | ModifierType::Weld
                | ModifierType::Triangulate
                | ModifierType::EdgeSplit
                | ModifierType::DataTransfer
                | ModifierType::Displace
                | ModifierType::Shrinkwrap
                | ModifierType::Armature
                | ModifierType::SmoothByAngle
        )
    }

    #[must_use]
    pub fn is_deform_only(self) -> bool {
        matches!(
            self,
            ModifierType::Displace | ModifierType::Shrinkwrap | ModifierType::Armature
        )
    }

    #[must_use]
    pub fn is_non_geometrical(self) -> bool {
        matches!(self, ModifierType::DataTransfer)
    }
}

impl ModifierKind {
    #[must_use]
    pub fn modifier_type(&self) -> ModifierType {
        match self {
            ModifierKind::Mirror(_) => ModifierType::Mirror,
            ModifierKind::Subdivision(_) => ModifierType::Subdivision,
            ModifierKind::Solidify(_) => ModifierType::Solidify,
            ModifierKind::Array(_) => ModifierType::Array,
            ModifierKind::Weld(_) => ModifierType::Weld,
            ModifierKind::Triangulate(_) => ModifierType::Triangulate,
            ModifierKind::EdgeSplit(_) => ModifierType::EdgeSplit,
            ModifierKind::Displace(_) => ModifierType::Displace,
            ModifierKind::Shrinkwrap(_) => ModifierType::Shrinkwrap,
            ModifierKind::DataTransfer(_) => ModifierType::DataTransfer,
            ModifierKind::SmoothByAngle(_) => ModifierType::SmoothByAngle,
            ModifierKind::Armature(_) => ModifierType::Armature,
            ModifierKind::Bevel(_) => ModifierType::Bevel,
            ModifierKind::Boolean(_) => ModifierType::Boolean,
            ModifierKind::Decimate(_) => ModifierType::Decimate,
        }
    }
}

macro_rules! kind_from {
    ($($variant:ident),*) => {
        $(
            impl From<$variant> for ModifierKind {
                fn from(value: $variant) -> ModifierKind {
                    ModifierKind::$variant(value)
                }
            }
        )*
    };
}

kind_from!(
    Mirror,
    Subdivision,
    Solidify,
    Array,
    Weld,
    Triangulate,
    EdgeSplit,
    Displace,
    Shrinkwrap,
    DataTransfer,
    SmoothByAngle,
    Armature,
    Bevel,
    Boolean,
    Decimate
);

#[derive(Clone, Debug, PartialEq)]
pub struct Modifier {
    pub name: String,
    pub show_viewport: bool,
    pub show_render: bool,
    pub show_in_editmode: bool,
    pub show_on_cage: bool,
    pub show_expanded: bool,
    pub pin_to_last: bool,
    pub kind: ModifierKind,
}

impl Modifier {
    #[must_use]
    pub fn new(name: impl Into<String>, kind: impl Into<ModifierKind>) -> Modifier {
        let kind = kind.into();
        Modifier {
            name: name.into(),
            show_viewport: true,
            show_render: true,
            show_in_editmode: kind.modifier_type().enabled_in_edit_mode(),
            show_on_cage: false,
            show_expanded: true,
            pin_to_last: false,
            kind,
        }
    }

    #[must_use]
    pub fn modifier_type(&self) -> ModifierType {
        self.kind.modifier_type()
    }

    #[must_use]
    pub fn is_enabled(&self, purpose: Purpose) -> bool {
        match purpose {
            Purpose::Viewport => self.show_viewport,
            Purpose::Render => self.show_render,
            Purpose::EditMode | Purpose::EditCage => {
                self.show_viewport
                    && self.show_in_editmode
                    && self.modifier_type().supports_edit_mode()
            }
        }
    }
}

tagged!(Purpose, "evaluation purpose" {
    Viewport = 0,
    Render = 1,
    EditMode = 2,
    EditCage = 3,
});

impl Codec for ModifierKind {
    type Error = DecodeError;

    fn encode(&self, out: &mut Writer<'_>) {
        match self {
            ModifierKind::Mirror(value) => {
                out.tag(0);
                out.put(value);
            }
            ModifierKind::Subdivision(value) => {
                out.tag(1);
                out.put(value);
            }
            ModifierKind::Solidify(value) => {
                out.tag(2);
                out.put(value);
            }
            ModifierKind::Array(value) => {
                out.tag(3);
                out.put(value);
            }
            ModifierKind::Weld(value) => {
                out.tag(4);
                out.put(value);
            }
            ModifierKind::Triangulate(value) => {
                out.tag(5);
                out.put(value);
            }
            ModifierKind::EdgeSplit(value) => {
                out.tag(6);
                out.put(value);
            }
            ModifierKind::Displace(value) => {
                out.tag(7);
                out.put(value);
            }
            ModifierKind::Shrinkwrap(value) => {
                out.tag(8);
                out.put(value);
            }
            ModifierKind::DataTransfer(value) => {
                out.tag(9);
                out.put(value);
            }
            ModifierKind::SmoothByAngle(value) => {
                out.tag(10);
                out.put(value);
            }
            ModifierKind::Armature(value) => {
                out.tag(11);
                out.put(value);
            }
            ModifierKind::Bevel(value) => {
                out.tag(12);
                out.put(value);
            }
            ModifierKind::Boolean(value) => {
                out.tag(13);
                out.put(value);
            }
            ModifierKind::Decimate(value) => {
                out.tag(14);
                out.put(value);
            }
        }
    }

    fn decode(input: &mut Reader<'_>) -> Result<Self, DecodeError> {
        Ok(match input.tag()? {
            0 => ModifierKind::Mirror(input.get()?),
            1 => ModifierKind::Subdivision(input.get()?),
            2 => ModifierKind::Solidify(input.get()?),
            3 => ModifierKind::Array(input.get()?),
            4 => ModifierKind::Weld(input.get()?),
            5 => ModifierKind::Triangulate(input.get()?),
            6 => ModifierKind::EdgeSplit(input.get()?),
            7 => ModifierKind::Displace(input.get()?),
            8 => ModifierKind::Shrinkwrap(input.get()?),
            9 => ModifierKind::DataTransfer(input.get()?),
            10 => ModifierKind::SmoothByAngle(input.get()?),
            11 => ModifierKind::Armature(input.get()?),
            12 => ModifierKind::Bevel(input.get()?),
            13 => ModifierKind::Boolean(input.get()?),
            14 => ModifierKind::Decimate(input.get()?),
            _ => return Err(DecodeError::Malformed("unknown modifier type")),
        })
    }
}

record!(Modifier {
    name,
    show_viewport,
    show_render,
    show_in_editmode,
    show_on_cage,
    show_expanded,
    pin_to_last,
    kind,
});
