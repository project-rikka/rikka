use crate::blend::BlendMode;
use crate::color::Color;
use crate::descriptor::{Descriptor, Value};
use crate::info::{DescriptorBlock, Info};
use crate::key::Key;
use crate::style::contour::Contour;
use crate::style::fields::{
    angle, blend_mode, blend_value, color, color_value, enumeration, flag, number, percent,
    percent_value, pixels,
};
use crate::style::fill::{FillContent, GradientFill, PatternFill};
use crate::style::gradient::Gradient;

#[derive(Debug, Clone, PartialEq)]
pub struct Common {
    pub enabled: bool,
    pub present: bool,
    pub show_in_dialog: bool,
    pub blend_mode: BlendMode,
    pub opacity: f64,
}

impl Common {
    fn read(descriptor: &Descriptor, mode_key: &str) -> Self {
        Self {
            enabled: flag(descriptor, "enab", true),
            present: flag(descriptor, "present", true),
            show_in_dialog: flag(descriptor, "showInDialog", true),
            blend_mode: blend_mode(descriptor, mode_key),
            opacity: percent(descriptor, "Opct", 1.0),
        }
    }

    fn write(&self, descriptor: &mut Descriptor, mode_key: &str) {
        descriptor.set("enab", self.enabled);
        descriptor.set("present", self.present);
        descriptor.set("showInDialog", self.show_in_dialog);
        descriptor.set(mode_key, blend_value(self.blend_mode));
        descriptor.set("Opct", percent_value(self.opacity));
    }

    fn with(blend_mode: BlendMode, opacity: f64) -> Self {
        Self {
            enabled: true,
            present: true,
            show_in_dialog: true,
            blend_mode,
            opacity,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Shadow {
    pub common: Common,
    pub color: Color,
    pub use_global_light: bool,
    pub angle: f64,
    pub distance: f64,
    pub spread: f64,
    pub size: f64,
    pub noise: f64,
    pub anti_aliased: bool,
    pub contour: Contour,
    pub layer_knocks_out: bool,
    source: Descriptor,
}

impl Shadow {
    pub fn new(color: Color) -> Self {
        Self {
            common: Common::with(BlendMode::Multiply, 0.75),
            color,
            use_global_light: true,
            angle: 120.0,
            distance: 5.0,
            spread: 0.0,
            size: 5.0,
            noise: 0.0,
            anti_aliased: false,
            contour: Contour::linear(),
            layer_knocks_out: true,
            source: Descriptor::new("DrSh"),
        }
    }

    fn read(descriptor: &Descriptor) -> Self {
        Self {
            common: Common::read(descriptor, "Md  "),
            color: color(descriptor, "Clr ").unwrap_or_default(),
            use_global_light: flag(descriptor, "uglg", true),
            angle: number(descriptor, "lagl", 120.0),
            distance: number(descriptor, "Dstn", 5.0),
            spread: percent(descriptor, "Ckmt", 0.0),
            size: number(descriptor, "blur", 5.0),
            noise: percent(descriptor, "Nose", 0.0),
            anti_aliased: flag(descriptor, "AntA", false),
            contour: descriptor
                .descriptor("TrnS")
                .map(Contour::from_descriptor)
                .unwrap_or_default(),
            layer_knocks_out: flag(descriptor, "layerConceals", true),
            source: descriptor.clone(),
        }
    }

    fn write(&self, inner: bool) -> Descriptor {
        let mut descriptor = self.source.clone();
        descriptor.class = crate::descriptor::Id::from(if inner { "IrSh" } else { "DrSh" });
        self.common.write(&mut descriptor, "Md  ");
        descriptor.set("Clr ", color_value(&self.color));
        descriptor.set("uglg", self.use_global_light);
        descriptor.set("lagl", angle(self.angle));
        descriptor.set("Dstn", pixels(self.distance));
        descriptor.set("Ckmt", percent_value(self.spread));
        descriptor.set("blur", pixels(self.size));
        descriptor.set("Nose", percent_value(self.noise));
        descriptor.set("AntA", self.anti_aliased);
        descriptor.set("TrnS", Value::Descriptor(self.contour.to_descriptor()));
        if !inner {
            descriptor.set("layerConceals", self.layer_knocks_out);
        }
        descriptor
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlowTechnique {
    Softer,
    Precise,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlowSource {
    Edge,
    Center,
}

#[derive(Debug, Clone, PartialEq)]
pub enum GlowPaint {
    Color(Color),
    Gradient(Gradient),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Glow {
    pub common: Common,
    pub paint: GlowPaint,
    pub technique: GlowTechnique,
    pub source: GlowSource,
    pub spread: f64,
    pub size: f64,
    pub noise: f64,
    pub jitter: f64,
    pub range: f64,
    pub anti_aliased: bool,
    pub contour: Contour,
    origin: Descriptor,
}

impl Glow {
    pub fn new(color: Color) -> Self {
        Self {
            common: Common::with(BlendMode::Screen, 0.75),
            paint: GlowPaint::Color(color),
            technique: GlowTechnique::Softer,
            source: GlowSource::Edge,
            spread: 0.0,
            size: 5.0,
            noise: 0.0,
            jitter: 0.0,
            range: 0.5,
            anti_aliased: false,
            contour: Contour::linear(),
            origin: Descriptor::new("OrGl"),
        }
    }

    fn read(descriptor: &Descriptor) -> Self {
        let paint = descriptor
            .descriptor("Grad")
            .map(|gradient| GlowPaint::Gradient(Gradient::from_descriptor(gradient)))
            .unwrap_or_else(|| GlowPaint::Color(color(descriptor, "Clr ").unwrap_or(Color::WHITE)));
        Self {
            common: Common::read(descriptor, "Md  "),
            paint,
            technique: if descriptor.enumeration("GlwT").as_deref() == Some("PrBL") {
                GlowTechnique::Precise
            } else {
                GlowTechnique::Softer
            },
            source: if descriptor.enumeration("glwS").as_deref() == Some("SrcC") {
                GlowSource::Center
            } else {
                GlowSource::Edge
            },
            spread: percent(descriptor, "Ckmt", 0.0),
            size: number(descriptor, "blur", 5.0),
            noise: percent(descriptor, "Nose", 0.0),
            jitter: percent(descriptor, "ShdN", 0.0),
            range: percent(descriptor, "Inpr", 0.5),
            anti_aliased: flag(descriptor, "AntA", false),
            contour: descriptor
                .descriptor("TrnS")
                .map(Contour::from_descriptor)
                .unwrap_or_default(),
            origin: descriptor.clone(),
        }
    }

    fn write(&self, inner: bool) -> Descriptor {
        let mut descriptor = self.origin.clone();
        descriptor.class = crate::descriptor::Id::from(if inner { "IrGl" } else { "OrGl" });
        self.common.write(&mut descriptor, "Md  ");
        match &self.paint {
            GlowPaint::Color(fill) => {
                descriptor.remove("Grad");
                descriptor.set("Clr ", color_value(fill));
            }
            GlowPaint::Gradient(gradient) => {
                descriptor.set("Grad", Value::Descriptor(gradient.to_descriptor()))
            }
        }
        descriptor.set(
            "GlwT",
            enumeration(
                "BETE",
                match self.technique {
                    GlowTechnique::Softer => "SfBL",
                    GlowTechnique::Precise => "PrBL",
                },
            ),
        );
        if inner {
            descriptor.set(
                "glwS",
                enumeration(
                    "IGSr",
                    match self.source {
                        GlowSource::Edge => "SrcE",
                        GlowSource::Center => "SrcC",
                    },
                ),
            );
        }
        descriptor.set("Ckmt", percent_value(self.spread));
        descriptor.set("blur", pixels(self.size));
        descriptor.set("Nose", percent_value(self.noise));
        descriptor.set("ShdN", percent_value(self.jitter));
        descriptor.set("Inpr", percent_value(self.range));
        descriptor.set("AntA", self.anti_aliased);
        descriptor.set("TrnS", Value::Descriptor(self.contour.to_descriptor()));
        descriptor
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BevelStyle {
    Outer,
    Inner,
    Emboss,
    PillowEmboss,
    StrokeEmboss,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BevelTechnique {
    Smooth,
    ChiselHard,
    ChiselSoft,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Bevel {
    pub enabled: bool,
    pub present: bool,
    pub style: BevelStyle,
    pub technique: BevelTechnique,
    pub depth: f64,
    pub up: bool,
    pub size: f64,
    pub soften: f64,
    pub use_global_light: bool,
    pub angle: f64,
    pub altitude: f64,
    pub gloss_contour: Contour,
    pub anti_alias_gloss: bool,
    pub highlight_mode: BlendMode,
    pub highlight_color: Color,
    pub highlight_opacity: f64,
    pub shadow_mode: BlendMode,
    pub shadow_color: Color,
    pub shadow_opacity: f64,
    pub use_shape: bool,
    pub shape_contour: Contour,
    pub shape_range: f64,
    pub use_texture: bool,
    pub texture: Option<PatternFill>,
    pub texture_depth: f64,
    pub texture_invert: bool,
    source: Descriptor,
}

impl Bevel {
    fn read(descriptor: &Descriptor) -> Self {
        let style = match descriptor.enumeration("bvlS").as_deref() {
            Some("OtrB") => BevelStyle::Outer,
            Some("Embs") => BevelStyle::Emboss,
            Some("PlEb") => BevelStyle::PillowEmboss,
            Some("strokeEmboss") => BevelStyle::StrokeEmboss,
            _ => BevelStyle::Inner,
        };
        let technique = match descriptor.enumeration("bvlT").as_deref() {
            Some("PrBL") => BevelTechnique::ChiselHard,
            Some("Slmt") => BevelTechnique::ChiselSoft,
            _ => BevelTechnique::Smooth,
        };
        let texture = if descriptor.contains("Ptrn") {
            PatternFill::from_descriptor(descriptor)
        } else {
            None
        };
        Self {
            enabled: flag(descriptor, "enab", true),
            present: flag(descriptor, "present", true),
            style,
            technique,
            depth: percent(descriptor, "srgR", 1.0),
            up: descriptor.enumeration("bvlD").as_deref() != Some("Out "),
            size: number(descriptor, "blur", 5.0),
            soften: number(descriptor, "Sftn", 0.0),
            use_global_light: flag(descriptor, "uglg", true),
            angle: number(descriptor, "lagl", 120.0),
            altitude: number(descriptor, "Lald", 30.0),
            gloss_contour: descriptor
                .descriptor("TrnS")
                .map(Contour::from_descriptor)
                .unwrap_or_default(),
            anti_alias_gloss: flag(descriptor, "antialiasGloss", false),
            highlight_mode: blend_mode(descriptor, "hglM"),
            highlight_color: color(descriptor, "hglC").unwrap_or(Color::WHITE),
            highlight_opacity: percent(descriptor, "hglO", 0.75),
            shadow_mode: blend_mode(descriptor, "sdwM"),
            shadow_color: color(descriptor, "sdwC").unwrap_or(Color::BLACK),
            shadow_opacity: percent(descriptor, "sdwO", 0.75),
            use_shape: flag(descriptor, "useShape", false),
            shape_contour: descriptor
                .descriptor("MpgS")
                .map(Contour::from_descriptor)
                .unwrap_or_default(),
            shape_range: percent(descriptor, "Inpr", 0.5),
            use_texture: flag(descriptor, "useTexture", false),
            texture,
            texture_depth: percent(descriptor, "textureDepth", 1.0),
            texture_invert: flag(descriptor, "InvT", false),
            source: descriptor.clone(),
        }
    }

    fn write(&self) -> Descriptor {
        let mut descriptor = self.source.clone();
        descriptor.class = crate::descriptor::Id::from("ebbl");
        descriptor.set("enab", self.enabled);
        descriptor.set("present", self.present);
        descriptor.set("hglM", blend_value(self.highlight_mode));
        descriptor.set("hglC", color_value(&self.highlight_color));
        descriptor.set("hglO", percent_value(self.highlight_opacity));
        descriptor.set("sdwM", blend_value(self.shadow_mode));
        descriptor.set("sdwC", color_value(&self.shadow_color));
        descriptor.set("sdwO", percent_value(self.shadow_opacity));
        descriptor.set(
            "bvlT",
            enumeration(
                "bvlT",
                match self.technique {
                    BevelTechnique::Smooth => "SfBL",
                    BevelTechnique::ChiselHard => "PrBL",
                    BevelTechnique::ChiselSoft => "Slmt",
                },
            ),
        );
        descriptor.set(
            "bvlS",
            enumeration(
                "BESl",
                match self.style {
                    BevelStyle::Outer => "OtrB",
                    BevelStyle::Inner => "InrB",
                    BevelStyle::Emboss => "Embs",
                    BevelStyle::PillowEmboss => "PlEb",
                    BevelStyle::StrokeEmboss => "strokeEmboss",
                },
            ),
        );
        descriptor.set("uglg", self.use_global_light);
        descriptor.set("lagl", angle(self.angle));
        descriptor.set("Lald", angle(self.altitude));
        descriptor.set("srgR", percent_value(self.depth));
        descriptor.set("blur", pixels(self.size));
        descriptor.set(
            "bvlD",
            enumeration("BESs", if self.up { "In  " } else { "Out " }),
        );
        descriptor.set(
            "TrnS",
            Value::Descriptor(self.gloss_contour.to_descriptor()),
        );
        descriptor.set("antialiasGloss", self.anti_alias_gloss);
        descriptor.set("Sftn", pixels(self.soften));
        descriptor.set("useShape", self.use_shape);
        descriptor.set(
            "MpgS",
            Value::Descriptor(self.shape_contour.to_descriptor()),
        );
        descriptor.set("Inpr", percent_value(self.shape_range));
        descriptor.set("useTexture", self.use_texture);
        if let Some(texture) = &self.texture {
            texture.write(&mut descriptor);
            descriptor.set("textureDepth", percent_value(self.texture_depth));
            descriptor.set("InvT", self.texture_invert);
        }
        descriptor
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Satin {
    pub common: Common,
    pub color: Color,
    pub angle: f64,
    pub distance: f64,
    pub size: f64,
    pub invert: bool,
    pub anti_aliased: bool,
    pub contour: Contour,
    source: Descriptor,
}

impl Satin {
    fn read(descriptor: &Descriptor) -> Self {
        Self {
            common: Common::read(descriptor, "Md  "),
            color: color(descriptor, "Clr ").unwrap_or_default(),
            angle: number(descriptor, "lagl", 19.0),
            distance: number(descriptor, "Dstn", 11.0),
            size: number(descriptor, "blur", 14.0),
            invert: flag(descriptor, "Invr", true),
            anti_aliased: flag(descriptor, "AntA", true),
            contour: descriptor
                .descriptor("MpgS")
                .map(Contour::from_descriptor)
                .unwrap_or_default(),
            source: descriptor.clone(),
        }
    }

    fn write(&self) -> Descriptor {
        let mut descriptor = self.source.clone();
        descriptor.class = crate::descriptor::Id::from("ChFX");
        self.common.write(&mut descriptor, "Md  ");
        descriptor.set("Clr ", color_value(&self.color));
        descriptor.set("lagl", angle(self.angle));
        descriptor.set("Dstn", pixels(self.distance));
        descriptor.set("blur", pixels(self.size));
        descriptor.set("Invr", self.invert);
        descriptor.set("AntA", self.anti_aliased);
        descriptor.set("MpgS", Value::Descriptor(self.contour.to_descriptor()));
        descriptor
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Overlay {
    pub common: Common,
    pub content: FillContent,
    source: Descriptor,
}

impl Overlay {
    pub fn color(color: Color) -> Self {
        Self {
            common: Common::with(BlendMode::Normal, 1.0),
            content: FillContent::Solid(color),
            source: Descriptor::new("SoFi"),
        }
    }

    pub fn gradient(fill: GradientFill) -> Self {
        Self {
            common: Common::with(BlendMode::Normal, 1.0),
            content: FillContent::Gradient(fill),
            source: Descriptor::new("GrFl"),
        }
    }

    fn read(descriptor: &Descriptor) -> Option<Self> {
        Some(Self {
            common: Common::read(descriptor, "Md  "),
            content: FillContent::from_descriptor(descriptor)?,
            source: descriptor.clone(),
        })
    }

    fn write(&self) -> Descriptor {
        let mut descriptor = self.source.clone();
        descriptor.class = crate::descriptor::Id::from(match self.content {
            FillContent::Solid(_) => "SoFi",
            FillContent::Gradient(_) => "GrFl",
            FillContent::Pattern(_) => "patternFill",
        });
        self.common.write(&mut descriptor, "Md  ");
        self.content.write(&mut descriptor);
        descriptor
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StrokePosition {
    Outside,
    Inside,
    Center,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Stroke {
    pub common: Common,
    pub position: StrokePosition,
    pub size: f64,
    pub overprint: bool,
    pub content: FillContent,
    source: Descriptor,
}

impl Stroke {
    pub fn new(size: f64, color: Color) -> Self {
        Self {
            common: Common::with(BlendMode::Normal, 1.0),
            position: StrokePosition::Outside,
            size,
            overprint: false,
            content: FillContent::Solid(color),
            source: Descriptor::new("FrFX"),
        }
    }

    fn read(descriptor: &Descriptor) -> Self {
        let content = match descriptor.enumeration("PntT").as_deref() {
            Some("GrFl") => GradientFill::from_descriptor(descriptor).map(FillContent::Gradient),
            Some("Ptrn") => PatternFill::from_descriptor(descriptor).map(FillContent::Pattern),
            _ => None,
        }
        .unwrap_or_else(|| FillContent::Solid(color(descriptor, "Clr ").unwrap_or_default()));
        Self {
            common: Common::read(descriptor, "Md  "),
            position: match descriptor.enumeration("Styl").as_deref() {
                Some("InsF") => StrokePosition::Inside,
                Some("CtrF") => StrokePosition::Center,
                _ => StrokePosition::Outside,
            },
            size: number(descriptor, "Sz  ", 3.0),
            overprint: flag(descriptor, "overprint", false),
            content,
            source: descriptor.clone(),
        }
    }

    fn write(&self) -> Descriptor {
        let mut descriptor = self.source.clone();
        descriptor.class = crate::descriptor::Id::from("FrFX");
        self.common.write(&mut descriptor, "Md  ");
        descriptor.set(
            "Styl",
            enumeration(
                "FStl",
                match self.position {
                    StrokePosition::Outside => "OutF",
                    StrokePosition::Inside => "InsF",
                    StrokePosition::Center => "CtrF",
                },
            ),
        );
        descriptor.set(
            "PntT",
            enumeration(
                "FrFl",
                match self.content {
                    FillContent::Solid(_) => "SClr",
                    FillContent::Gradient(_) => "GrFl",
                    FillContent::Pattern(_) => "Ptrn",
                },
            ),
        );
        descriptor.set("Sz  ", pixels(self.size));
        descriptor.set("overprint", self.overprint);
        self.content.write(&mut descriptor);
        descriptor
    }
}

const MULTI_KEYS: [&str; 5] = [
    "dropShadowMulti",
    "innerShadowMulti",
    "solidFillMulti",
    "gradientFillMulti",
    "frameFXMulti",
];

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Effects {
    pub enabled: bool,
    pub scale: f64,
    pub drop_shadows: Vec<Shadow>,
    pub inner_shadows: Vec<Shadow>,
    pub outer_glows: Vec<Glow>,
    pub inner_glows: Vec<Glow>,
    pub bevels: Vec<Bevel>,
    pub satins: Vec<Satin>,
    pub color_overlays: Vec<Overlay>,
    pub gradient_overlays: Vec<Overlay>,
    pub pattern_overlays: Vec<Overlay>,
    pub strokes: Vec<Stroke>,
    source: Descriptor,
}

fn entries<'a>(descriptor: &'a Descriptor, single: &str, multi: &str) -> Vec<&'a Descriptor> {
    if let Some(items) = descriptor.list(multi) {
        return items.iter().filter_map(Value::as_descriptor).collect();
    }
    descriptor.descriptor(single).into_iter().collect()
}

impl Effects {
    pub fn new() -> Self {
        Self {
            enabled: true,
            scale: 1.0,
            source: Descriptor::new("null"),
            ..Self::default()
        }
    }

    pub fn from_descriptor(descriptor: &Descriptor) -> Self {
        Self {
            enabled: flag(descriptor, "masterFXSwitch", true),
            scale: percent(descriptor, "Scl ", 1.0),
            drop_shadows: entries(descriptor, "DrSh", "dropShadowMulti")
                .into_iter()
                .map(Shadow::read)
                .collect(),
            inner_shadows: entries(descriptor, "IrSh", "innerShadowMulti")
                .into_iter()
                .map(Shadow::read)
                .collect(),
            outer_glows: entries(descriptor, "OrGl", "outerGlowMulti")
                .into_iter()
                .map(Glow::read)
                .collect(),
            inner_glows: entries(descriptor, "IrGl", "innerGlowMulti")
                .into_iter()
                .map(Glow::read)
                .collect(),
            bevels: entries(descriptor, "ebbl", "bevelEmbossMulti")
                .into_iter()
                .map(Bevel::read)
                .collect(),
            satins: entries(descriptor, "ChFX", "chromeFXMulti")
                .into_iter()
                .map(Satin::read)
                .collect(),
            color_overlays: entries(descriptor, "SoFi", "solidFillMulti")
                .into_iter()
                .filter_map(Overlay::read)
                .collect(),
            gradient_overlays: entries(descriptor, "GrFl", "gradientFillMulti")
                .into_iter()
                .filter_map(Overlay::read)
                .collect(),
            pattern_overlays: entries(descriptor, "patternFill", "patternFillMulti")
                .into_iter()
                .filter_map(Overlay::read)
                .collect(),
            strokes: entries(descriptor, "FrFX", "frameFXMulti")
                .into_iter()
                .map(Stroke::read)
                .collect(),
            source: descriptor.clone(),
        }
    }

    pub fn from_layer_info(info: &[Info]) -> Option<Self> {
        let preferred = info.iter().find_map(|item| match item {
            Info::Descriptor(block) if block.key.0 == *b"lmfx" => Some(&block.descriptor),
            _ => None,
        });
        let fallback = info.iter().find_map(|item| match item {
            Info::Descriptor(block) if matches!(&block.key.0, b"lfx2" | b"lfxs") => {
                Some(&block.descriptor)
            }
            _ => None,
        });
        preferred.or(fallback).map(Effects::from_descriptor)
    }

    pub fn is_multi(&self) -> bool {
        self.drop_shadows.len() > 1
            || self.inner_shadows.len() > 1
            || self.color_overlays.len() > 1
            || self.gradient_overlays.len() > 1
            || self.strokes.len() > 1
    }

    pub fn has_visible(&self) -> bool {
        self.enabled
            && (self.drop_shadows.iter().any(|e| e.common.enabled)
                || self.inner_shadows.iter().any(|e| e.common.enabled)
                || self.outer_glows.iter().any(|e| e.common.enabled)
                || self.inner_glows.iter().any(|e| e.common.enabled)
                || self.bevels.iter().any(|e| e.enabled)
                || self.satins.iter().any(|e| e.common.enabled)
                || self.color_overlays.iter().any(|e| e.common.enabled)
                || self.gradient_overlays.iter().any(|e| e.common.enabled)
                || self.pattern_overlays.iter().any(|e| e.common.enabled)
                || self.strokes.iter().any(|e| e.common.enabled))
    }

    pub fn scale_sizes(&mut self, factor: f64) {
        for shadow in self.drop_shadows.iter_mut().chain(&mut self.inner_shadows) {
            shadow.distance *= factor;
            shadow.size *= factor;
        }
        for glow in self.outer_glows.iter_mut().chain(&mut self.inner_glows) {
            glow.size *= factor;
        }
        for bevel in &mut self.bevels {
            bevel.size *= factor;
            bevel.soften *= factor;
            if let Some(texture) = &mut bevel.texture {
                texture.scale *= factor;
            }
        }
        for satin in &mut self.satins {
            satin.distance *= factor;
            satin.size *= factor;
        }
        for stroke in &mut self.strokes {
            stroke.size *= factor;
        }
        let patterns = self
            .pattern_overlays
            .iter_mut()
            .map(|overlay| &mut overlay.content)
            .chain(self.strokes.iter_mut().map(|stroke| &mut stroke.content));
        for content in patterns {
            if let FillContent::Pattern(pattern) = content {
                pattern.scale *= factor;
            }
        }
    }

    pub fn to_descriptor(&self) -> Descriptor {
        self.write(self.is_multi())
    }

    pub(crate) fn to_source_layout(&self) -> Descriptor {
        let multi = MULTI_KEYS.iter().any(|key| self.source.contains(key));
        self.write(multi || self.is_multi())
    }

    fn write(&self, multi: bool) -> Descriptor {
        let mut descriptor = self.source.clone();
        descriptor.set("Scl ", percent_value(self.scale));
        descriptor.set("masterFXSwitch", self.enabled);
        let mut place = |single: &str, plural: &str, items: Vec<Descriptor>| {
            descriptor.remove(single);
            descriptor.remove(plural);
            if items.is_empty() {
                return;
            }
            if multi && MULTI_KEYS.contains(&plural) {
                descriptor.set(
                    plural,
                    Value::List(items.into_iter().map(Value::Descriptor).collect()),
                );
            } else if let Some(first) = items.into_iter().next() {
                descriptor.set(single, Value::Descriptor(first));
            }
        };
        place(
            "DrSh",
            "dropShadowMulti",
            self.drop_shadows.iter().map(|e| e.write(false)).collect(),
        );
        place(
            "IrSh",
            "innerShadowMulti",
            self.inner_shadows.iter().map(|e| e.write(true)).collect(),
        );
        place(
            "OrGl",
            "outerGlowMulti",
            self.outer_glows.iter().map(|e| e.write(false)).collect(),
        );
        place(
            "IrGl",
            "innerGlowMulti",
            self.inner_glows.iter().map(|e| e.write(true)).collect(),
        );
        place(
            "ebbl",
            "bevelEmbossMulti",
            self.bevels.iter().map(Bevel::write).collect(),
        );
        place(
            "ChFX",
            "chromeFXMulti",
            self.satins.iter().map(Satin::write).collect(),
        );
        place(
            "SoFi",
            "solidFillMulti",
            self.color_overlays.iter().map(Overlay::write).collect(),
        );
        place(
            "GrFl",
            "gradientFillMulti",
            self.gradient_overlays.iter().map(Overlay::write).collect(),
        );
        place(
            "patternFill",
            "patternFillMulti",
            self.pattern_overlays.iter().map(Overlay::write).collect(),
        );
        place(
            "FrFX",
            "frameFXMulti",
            self.strokes.iter().map(Stroke::write).collect(),
        );
        descriptor
    }

    pub fn to_info(&self) -> Info {
        let key = Key(if self.is_multi() { *b"lmfx" } else { *b"lfx2" });
        Info::Descriptor(DescriptorBlock::new(key, self.to_descriptor()))
    }
}
