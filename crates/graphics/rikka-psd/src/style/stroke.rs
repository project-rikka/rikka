use crate::blend::BlendMode;
use crate::descriptor::{Descriptor, Value};
use crate::style::fields::{
    blend_mode, blend_value, enumeration, flag, number, percent, percent_value, pixels,
};
use crate::style::fill::FillContent;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineCap {
    Butt,
    Round,
    Square,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineJoin {
    Miter,
    Round,
    Bevel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineAlignment {
    Inside,
    Center,
    Outside,
}

#[derive(Debug, Clone, PartialEq)]
pub struct VectorStroke {
    pub stroke_enabled: bool,
    pub fill_enabled: bool,
    pub width: f64,
    pub dash_offset: f64,
    pub dashes: Vec<f64>,
    pub miter_limit: f64,
    pub cap: LineCap,
    pub join: LineJoin,
    pub alignment: LineAlignment,
    pub blend_mode: BlendMode,
    pub opacity: f64,
    pub content: Option<FillContent>,
    source: Descriptor,
}

impl VectorStroke {
    pub fn from_descriptor(descriptor: &Descriptor) -> Self {
        let cap = match descriptor.enumeration("strokeStyleLineCapType").as_deref() {
            Some("strokeStyleRoundCap") => LineCap::Round,
            Some("strokeStyleSquareCap") => LineCap::Square,
            _ => LineCap::Butt,
        };
        let join = match descriptor.enumeration("strokeStyleLineJoinType").as_deref() {
            Some("strokeStyleRoundJoin") => LineJoin::Round,
            Some("strokeStyleBevelJoin") => LineJoin::Bevel,
            _ => LineJoin::Miter,
        };
        let alignment = match descriptor
            .enumeration("strokeStyleLineAlignment")
            .as_deref()
        {
            Some("strokeStyleAlignInside") => LineAlignment::Inside,
            Some("strokeStyleAlignOutside") => LineAlignment::Outside,
            _ => LineAlignment::Center,
        };
        Self {
            stroke_enabled: flag(descriptor, "strokeEnabled", true),
            fill_enabled: flag(descriptor, "fillEnabled", true),
            width: number(descriptor, "strokeStyleLineWidth", 1.0),
            dash_offset: number(descriptor, "strokeStyleLineDashOffset", 0.0),
            dashes: descriptor
                .list("strokeStyleLineDashSet")
                .unwrap_or(&[])
                .iter()
                .filter_map(Value::number)
                .collect(),
            miter_limit: number(descriptor, "strokeStyleMiterLimit", 100.0),
            cap,
            join,
            alignment,
            blend_mode: blend_mode(descriptor, "strokeStyleBlendMode"),
            opacity: percent(descriptor, "strokeStyleOpacity", 1.0),
            content: descriptor
                .descriptor("strokeStyleContent")
                .and_then(FillContent::from_descriptor),
            source: descriptor.clone(),
        }
    }

    pub fn to_descriptor(&self) -> Descriptor {
        let mut descriptor = self.source.clone();
        descriptor.set("strokeEnabled", self.stroke_enabled);
        descriptor.set("fillEnabled", self.fill_enabled);
        descriptor.set("strokeStyleLineWidth", pixels(self.width));
        descriptor.set(
            "strokeStyleLineDashOffset",
            Value::UnitDouble(crate::descriptor::Unit::POINTS, self.dash_offset),
        );
        descriptor.set("strokeStyleMiterLimit", self.miter_limit);
        descriptor.set(
            "strokeStyleLineCapType",
            enumeration(
                "strokeStyleLineCapType",
                match self.cap {
                    LineCap::Butt => "strokeStyleButtCap",
                    LineCap::Round => "strokeStyleRoundCap",
                    LineCap::Square => "strokeStyleSquareCap",
                },
            ),
        );
        descriptor.set(
            "strokeStyleLineJoinType",
            enumeration(
                "strokeStyleLineJoinType",
                match self.join {
                    LineJoin::Miter => "strokeStyleMiterJoin",
                    LineJoin::Round => "strokeStyleRoundJoin",
                    LineJoin::Bevel => "strokeStyleBevelJoin",
                },
            ),
        );
        descriptor.set(
            "strokeStyleLineAlignment",
            enumeration(
                "strokeStyleLineAlignment",
                match self.alignment {
                    LineAlignment::Inside => "strokeStyleAlignInside",
                    LineAlignment::Center => "strokeStyleAlignCenter",
                    LineAlignment::Outside => "strokeStyleAlignOutside",
                },
            ),
        );
        descriptor.set(
            "strokeStyleLineDashSet",
            Value::List(
                self.dashes
                    .iter()
                    .map(|&value| Value::UnitDouble(crate::descriptor::Unit::NONE, value))
                    .collect(),
            ),
        );
        descriptor.set("strokeStyleBlendMode", blend_value(self.blend_mode));
        descriptor.set("strokeStyleOpacity", percent_value(self.opacity));
        if let Some(content) = &self.content {
            descriptor.set(
                "strokeStyleContent",
                Value::Descriptor(content.to_descriptor()),
            );
        }
        descriptor
    }
}
