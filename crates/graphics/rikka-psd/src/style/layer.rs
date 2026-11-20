use crate::descriptor::Value;
use crate::error::Result;
use crate::info::{Info, Knockout, PlacedKind, SectionKind, VectorMask};
use crate::key::Key;
use crate::layers::Layer;
use crate::style::adjustments::{AdjustmentLayer, BlackWhite, ColorLookup, Vibrance};
use crate::style::effects::Effects;
use crate::style::fill::FillContent;
use crate::style::stroke::VectorStroke;
use crate::style::text::TextLayer;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayerKind {
    Pixel,
    GroupStart,
    GroupEnd,
    Text,
    Shape,
    Fill,
    Adjustment,
    SmartObject,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SmartObject {
    pub linked_id: String,
    pub unique_id: String,
    pub kind: PlacedKind,
    pub transform: [f64; 8],
    pub size: Option<(f64, f64)>,
}

const EFFECT_KEYS: [&[u8; 4]; 4] = [b"lfx2", b"lmfx", b"lfxs", b"lrFX"];

impl Layer {
    pub fn kind(&self) -> LayerKind {
        match self.section_kind() {
            SectionKind::OpenFolder | SectionKind::ClosedFolder => return LayerKind::GroupStart,
            SectionKind::BoundingDivider => return LayerKind::GroupEnd,
            SectionKind::Other => {}
        }
        if self.find(b"TySh").is_some() {
            return LayerKind::Text;
        }
        if self.find(b"SoLd").is_some()
            || self.find(b"SoLE").is_some()
            || self.find(b"PlLd").is_some()
            || self.find(b"plLd").is_some()
        {
            return LayerKind::SmartObject;
        }
        if self.is_adjustment() {
            return LayerKind::Adjustment;
        }
        if self.fill().is_some() {
            return if self.vector_mask().is_some()
                && (self.find(b"vogk").is_some() || self.find(b"vscg").is_some())
            {
                LayerKind::Shape
            } else {
                LayerKind::Fill
            };
        }
        if self.find(b"vscg").is_some() {
            return LayerKind::Shape;
        }
        LayerKind::Pixel
    }

    pub fn effects(&self) -> Option<Effects> {
        Effects::from_layer_info(&self.info)
    }

    pub fn set_effects(&mut self, effects: Option<&Effects>) {
        self.info
            .retain(|info| !EFFECT_KEYS.iter().any(|key| info.key().0 == **key));
        if let Some(effects) = effects {
            self.info.push(effects.to_info());
        }
    }

    pub fn fill(&self) -> Option<FillContent> {
        self.info.iter().find_map(|info| match info {
            Info::Descriptor(block)
                if matches!(&block.key.0, b"SoCo" | b"GdFl" | b"PtFl" | b"vscg") =>
            {
                FillContent::from_descriptor(&block.descriptor)
            }
            _ => None,
        })
    }

    pub fn set_fill(&mut self, fill: &FillContent) {
        self.info
            .retain(|info| !matches!(&info.key().0, b"SoCo" | b"GdFl" | b"PtFl"));
        self.info.push(Info::descriptor(
            Key(*fill.info_key()),
            fill.to_descriptor(),
        ));
    }

    pub fn vector_mask(&self) -> Option<&VectorMask> {
        self.info.iter().find_map(|info| match info {
            Info::VectorMask(mask) => Some(mask),
            _ => None,
        })
    }

    pub fn vector_stroke(&self) -> Option<VectorStroke> {
        self.descriptor(&[b"vstk"])
            .map(VectorStroke::from_descriptor)
    }

    pub fn text(&self) -> Option<Result<TextLayer>> {
        TextLayer::from_layer(self)
    }

    fn is_adjustment(&self) -> bool {
        self.info.iter().any(|info| match info {
            Info::Adjustment(_) => true,
            Info::Descriptor(block) => matches!(&block.key.0, b"vibA" | b"blwh" | b"clrL"),
            _ => false,
        })
    }

    pub fn adjustment(&self) -> Option<AdjustmentLayer> {
        self.info.iter().find_map(|info| match info {
            Info::Adjustment(adjustment) => Some(AdjustmentLayer::Binary(adjustment.clone())),
            Info::Descriptor(block) => match &block.key.0 {
                b"vibA" => Some(AdjustmentLayer::Vibrance(Vibrance::from_descriptor(
                    &block.descriptor,
                ))),
                b"blwh" => Some(AdjustmentLayer::BlackWhite(BlackWhite::from_descriptor(
                    &block.descriptor,
                ))),
                b"clrL" => Some(AdjustmentLayer::ColorLookup(ColorLookup::from_descriptor(
                    &block.descriptor,
                ))),
                _ => None,
            },
            _ => None,
        })
    }

    pub fn smart_object(&self) -> Option<SmartObject> {
        let placed = self.info.iter().find_map(|info| match info {
            Info::PlacedLayer(placed) => Some(placed),
            _ => None,
        });
        let descriptor = self.descriptor(&[b"SoLd", b"SoLE"]);
        if placed.is_none() && descriptor.is_none() {
            return None;
        }
        let mut object = SmartObject {
            linked_id: placed
                .map(|placed| placed.unique_id.clone())
                .unwrap_or_default(),
            unique_id: placed
                .map(|placed| placed.unique_id.clone())
                .unwrap_or_default(),
            kind: placed.map_or(PlacedKind::Unknown, |placed| placed.kind),
            transform: placed.map_or([0.0; 8], |placed| placed.transform),
            size: None,
        };
        if let Some(descriptor) = descriptor {
            if let Some(id) = descriptor.string("Idnt") {
                object.linked_id = id.to_owned();
            }
            if let Some(id) = descriptor.string("placed") {
                object.unique_id = id.to_owned();
            }
            if let Some(items) = descriptor.list("Trnf") {
                for (slot, value) in object
                    .transform
                    .iter_mut()
                    .zip(items.iter().filter_map(Value::number))
                {
                    *slot = value;
                }
            }
            if let Some(size) = descriptor.descriptor("Sz  ") {
                object.size = Some((
                    size.number("Wdth").unwrap_or(0.0),
                    size.number("Hght").unwrap_or(0.0),
                ));
            }
        }
        Some(object)
    }

    pub fn knockout(&self) -> Knockout {
        self.info
            .iter()
            .find_map(|info| match info {
                Info::Knockout(knockout) => Some(*knockout),
                _ => None,
            })
            .unwrap_or(Knockout::None)
    }

    fn flag_info(&self, pick: impl Fn(&Info) -> Option<bool>, fallback: bool) -> bool {
        self.info.iter().find_map(pick).unwrap_or(fallback)
    }

    pub fn blend_clipped_as_group(&self) -> bool {
        self.flag_info(
            |info| match info {
                Info::BlendClipped(value) => Some(*value),
                _ => None,
            },
            true,
        )
    }

    pub fn blend_interior_effects(&self) -> bool {
        self.flag_info(
            |info| match info {
                Info::BlendInterior(value) => Some(*value),
                _ => None,
            },
            false,
        )
    }

    pub fn transparency_shapes_layer(&self) -> bool {
        self.flag_info(
            |info| match info {
                Info::TransparencyShapes(value) => Some(*value),
                _ => None,
            },
            true,
        )
    }

    pub fn layer_mask_hides_effects(&self) -> bool {
        self.flag_info(
            |info| match info {
                Info::LayerMaskAsGlobal(value) => Some(*value),
                _ => None,
            },
            false,
        )
    }

    pub fn vector_mask_hides_effects(&self) -> bool {
        self.flag_info(
            |info| match info {
                Info::VectorMaskAsGlobal(value) => Some(*value),
                _ => None,
            },
            false,
        )
    }

    pub fn channel_restrictions(&self) -> &[i32] {
        self.info
            .iter()
            .find_map(|info| match info {
                Info::ChannelRestrictions(channels) => Some(channels.as_slice()),
                _ => None,
            })
            .unwrap_or(&[])
    }
}
