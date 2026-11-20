pub(crate) mod build;
pub(crate) mod filters;
pub(crate) mod glyphs;
pub(crate) mod images;
pub(crate) mod levels;
pub(crate) mod markers;
pub(crate) mod servers;
pub(crate) mod shapes;
pub(crate) mod style;
pub(crate) mod text;

use std::sync::Arc;

use rikka_text_font::{Color as GlyphColor, Synthesis};
use rikka_text_layout::{FaceId, LocationId};
use rikka_types::{Color, Rect};
use rikka_vector::{
    Antialiasing, BlendMode, Cap, FillRule, GradientShape, Join, Path, Pixmap, Spread, Stop,
    Transform,
};

use crate::parse::values::{AspectRatio, ViewBox};
use crate::tree::filters::Filter;

#[derive(Clone, Debug)]
pub(crate) struct Tree {
    pub(crate) width: f32,
    pub(crate) height: f32,
    pub(crate) root: Group,
}

#[derive(Clone, Debug)]
pub(crate) enum Node {
    Group(Group),
    Shape(Shape),
    Picture(Picture),
    Glyph(GlyphPicture),
}

#[derive(Clone, Debug)]
pub(crate) struct Group {
    pub(crate) id: String,
    pub(crate) transform: Transform,
    pub(crate) opacity: f32,
    pub(crate) blend: BlendMode,
    pub(crate) isolated: bool,
    pub(crate) clip: Option<Arc<Clip>>,
    pub(crate) mask: Option<Arc<Masking>>,
    pub(crate) filters: Vec<Arc<Filter>>,
    pub(crate) children: Vec<Node>,
}

impl Default for Group {
    fn default() -> Self {
        Group {
            id: String::new(),
            transform: Transform::IDENTITY,
            opacity: 1.0,
            blend: BlendMode::SourceOver,
            isolated: false,
            clip: None,
            mask: None,
            filters: Vec::new(),
            children: Vec::new(),
        }
    }
}

impl Group {
    pub(crate) fn is_plain(&self) -> bool {
        self.opacity >= 1.0
            && self.blend == BlendMode::SourceOver
            && !self.isolated
            && self.clip.is_none()
            && self.mask.is_none()
            && self.filters.is_empty()
    }

    pub(crate) fn paints_once(&self) -> bool {
        matches!(
            self.children.as_slice(),
            [Node::Shape(shape)] if shape.fill.is_some() != shape.stroke.is_some()
        )
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Shape {
    pub(crate) id: String,
    pub(crate) transform: Transform,
    pub(crate) path: Path,
    pub(crate) bounds: Option<Rect>,
    pub(crate) fill: Option<Fill>,
    pub(crate) stroke: Option<Outline>,
    pub(crate) stroke_first: bool,
    pub(crate) antialiasing: Antialiasing,
}

impl Shape {
    pub(crate) fn solid(path: Path) -> Shape {
        Shape {
            id: String::new(),
            transform: Transform::IDENTITY,
            path,
            bounds: None,
            fill: Some(Fill {
                ink: Ink::Color(Color::rgba(0.0, 0.0, 0.0, 1.0)),
                opacity: 1.0,
                rule: FillRule::NonZero,
            }),
            stroke: None,
            stroke_first: false,
            antialiasing: Antialiasing::Enabled,
        }
    }

    pub(crate) fn object(&self) -> Option<Rect> {
        self.bounds.or_else(|| self.path.bounds())
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Fill {
    pub(crate) ink: Ink,
    pub(crate) opacity: f32,
    pub(crate) rule: FillRule,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum StrokeSpace {
    User,
    Host,
}

#[derive(Clone, Debug)]
pub(crate) struct Outline {
    pub(crate) ink: Ink,
    pub(crate) opacity: f32,
    pub(crate) width: f32,
    pub(crate) join: Join,
    pub(crate) cap: Cap,
    pub(crate) miter_limit: f32,
    pub(crate) dashes: Vec<f32>,
    pub(crate) dash_offset: f32,
    pub(crate) space: StrokeSpace,
}

#[derive(Clone, Debug)]
pub(crate) enum Ink {
    Color(Color),
    Gradient(Arc<Gradient>),
    Pattern(Arc<Pattern>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Units {
    UserSpace,
    BoundingBox,
}

#[derive(Clone, Debug)]
pub(crate) struct Gradient {
    pub(crate) units: Units,
    pub(crate) transform: Transform,
    pub(crate) spread: Spread,
    pub(crate) stops: Vec<Stop>,
    pub(crate) shape: GradientShape,
}

#[derive(Clone, Debug)]
pub(crate) struct Pattern {
    pub(crate) units: Units,
    pub(crate) content_units: Units,
    pub(crate) transform: Transform,
    pub(crate) rect: Rect,
    pub(crate) view: Option<(ViewBox, AspectRatio)>,
    pub(crate) content: Group,
}

#[derive(Clone, Debug)]
pub(crate) struct Clip {
    pub(crate) units: Units,
    pub(crate) transform: Transform,
    pub(crate) clip: Option<Arc<Clip>>,
    pub(crate) content: Group,
}

impl Clip {
    pub(crate) fn window(path: Path) -> Clip {
        Clip {
            units: Units::UserSpace,
            transform: Transform::IDENTITY,
            clip: None,
            content: Group {
                children: vec![Node::Shape(Shape::solid(path))],
                ..Group::default()
            },
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Masking {
    pub(crate) units: Units,
    pub(crate) content_units: Units,
    pub(crate) rect: Rect,
    pub(crate) luminance: bool,
    pub(crate) linear: bool,
    pub(crate) mask: Option<Arc<Masking>>,
    pub(crate) content: Group,
}

#[derive(Clone, Debug)]
pub(crate) struct Picture {
    pub(crate) id: String,
    pub(crate) transform: Transform,
    pub(crate) frame: Rect,
    pub(crate) aspect: AspectRatio,
    pub(crate) smooth: bool,
    pub(crate) content: PictureContent,
}

#[derive(Clone, Debug)]
pub(crate) enum PictureContent {
    Raster(Arc<Pixmap>),
    Vector(Arc<Tree>),
}

#[derive(Clone, Debug)]
pub(crate) struct GlyphPicture {
    pub(crate) transform: Transform,
    pub(crate) extent: Rect,
    pub(crate) opacity: f32,
    pub(crate) content: GlyphContent,
}

#[derive(Clone, Debug)]
pub(crate) enum GlyphContent {
    Raster(RasterGlyph),
    Vector {
        node: Arc<Node>,
        placement: Transform,
    },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct RasterGlyph {
    pub(crate) face: FaceId,
    pub(crate) location: LocationId,
    pub(crate) id: u16,
    pub(crate) size: f32,
    pub(crate) synthesis: Synthesis,
    pub(crate) foreground: GlyphColor,
}
