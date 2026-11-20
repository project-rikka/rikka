use std::collections::HashMap;

use crate::error::{Error, Result};
use crate::parse::css::{Sheet, declarations};
use crate::parse::media::{ColorScheme, Media, MediaRules, QueryList};
use crate::parse::selectors::{Scope, Work};
use crate::parse::values::{Unit, length, view_box};

const SVG: &str = "http://www.w3.org/2000/svg";
const XML: &str = "http://www.w3.org/XML/1998/namespace";
const MAX_ELEMENTS: usize = 1 << 20;
const MAX_MARKUP: usize = 2 * MAX_ELEMENTS;
const FALLBACK_SIDE: f32 = 100.0;

const PRESENTATION: [&str; 59] = [
    "alignment-baseline",
    "baseline-shift",
    "clip-path",
    "clip-rule",
    "color",
    "color-interpolation",
    "color-interpolation-filters",
    "display",
    "dominant-baseline",
    "fill",
    "fill-opacity",
    "fill-rule",
    "filter",
    "flood-color",
    "flood-opacity",
    "font-family",
    "font-size",
    "font-stretch",
    "font-style",
    "font-variant",
    "font-weight",
    "image-rendering",
    "isolation",
    "letter-spacing",
    "lighting-color",
    "marker-end",
    "marker-mid",
    "marker-start",
    "mask",
    "mask-type",
    "mix-blend-mode",
    "opacity",
    "overflow",
    "paint-order",
    "shape-rendering",
    "stop-color",
    "stop-opacity",
    "stroke",
    "stroke-dasharray",
    "stroke-dashoffset",
    "stroke-linecap",
    "stroke-linejoin",
    "stroke-miterlimit",
    "stroke-opacity",
    "stroke-width",
    "text-anchor",
    "text-decoration",
    "text-rendering",
    "transform",
    "transform-origin",
    "vector-effect",
    "visibility",
    "white-space",
    "word-spacing",
    "writing-mode",
    "x",
    "y",
    "width",
    "height",
];

const GEOMETRY: [&str; 4] = ["x", "y", "width", "height"];
const MARKERS: [&str; 3] = ["marker-start", "marker-mid", "marker-end"];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Tag {
    Svg,
    Group,
    Defs,
    Symbol,
    Use,
    Switch,
    Path,
    Rect,
    Circle,
    Ellipse,
    Line,
    Polyline,
    Polygon,
    Image,
    Text,
    TextSpan,
    TextPath,
    Characters,
    Marker,
    LinearGradient,
    RadialGradient,
    Stop,
    Pattern,
    ClipPath,
    Mask,
    Filter,
    Primitive,
    PrimitivePart,
    Style,
    Other,
}

impl Tag {
    fn named(name: &str, in_text: bool) -> Tag {
        match name {
            "svg" => Tag::Svg,
            "a" if in_text => Tag::TextSpan,
            "g" | "a" => Tag::Group,
            "defs" => Tag::Defs,
            "symbol" => Tag::Symbol,
            "use" => Tag::Use,
            "switch" => Tag::Switch,
            "path" => Tag::Path,
            "rect" => Tag::Rect,
            "circle" => Tag::Circle,
            "ellipse" => Tag::Ellipse,
            "line" => Tag::Line,
            "polyline" => Tag::Polyline,
            "polygon" => Tag::Polygon,
            "image" => Tag::Image,
            "text" => Tag::Text,
            "tspan" if in_text => Tag::TextSpan,
            "textPath" if in_text => Tag::TextPath,
            "marker" => Tag::Marker,
            "linearGradient" => Tag::LinearGradient,
            "radialGradient" => Tag::RadialGradient,
            "stop" => Tag::Stop,
            "pattern" => Tag::Pattern,
            "clipPath" => Tag::ClipPath,
            "mask" => Tag::Mask,
            "filter" => Tag::Filter,
            "style" => Tag::Style,
            "feMergeNode" | "feFuncR" | "feFuncG" | "feFuncB" | "feFuncA" | "feDistantLight"
            | "fePointLight" | "feSpotLight" => Tag::PrimitivePart,
            name if name.starts_with("fe") => Tag::Primitive,
            _ => Tag::Other,
        }
    }

    pub(crate) fn holds_text(self) -> bool {
        matches!(self, Tag::Text | Tag::TextSpan | Tag::TextPath)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct NodeId(usize);

#[derive(Clone, Debug)]
pub(crate) struct Element {
    pub(crate) tag: Tag,
    pub(crate) name: String,
    pub(crate) parent: Option<NodeId>,
    pub(crate) children: Vec<NodeId>,
    pub(crate) characters: String,
    attributes: Vec<(String, String)>,
    previous: Option<NodeId>,
    ordinal: usize,
    siblings: usize,
}

impl Element {
    pub(crate) fn attribute(&self, name: &str) -> Option<&str> {
        self.attributes
            .iter()
            .find(|(own, _)| own == name)
            .map(|(_, value)| value.as_str())
    }

    fn set(&mut self, name: &str, value: &str) {
        match self.attributes.iter_mut().find(|(own, _)| own == name) {
            Some((_, own)) => value.clone_into(own),
            None => self.attributes.push((name.to_owned(), value.to_owned())),
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Dom {
    elements: Vec<Element>,
    ids: HashMap<String, Vec<NodeId>>,
    queries: Vec<QueryList>,
    scheme: ColorScheme,
}

impl Dom {
    pub(crate) fn parse(
        source: &str,
        scheme: ColorScheme,
        viewport: Option<(f32, f32)>,
    ) -> Result<Dom> {
        if source.bytes().filter(|byte| *byte == b'<').count() > MAX_MARKUP {
            return Err(Error::TooComplex);
        }
        let root = rikka_xml::parse_lenient(source)
            .ok_or_else(|| Error::Malformed("no root element".to_owned()))?;
        if root.name() != "svg" {
            return Err(Error::Malformed(format!(
                "root element is `{}`, not `svg`",
                root.name()
            )));
        }
        let mut sheet = Sheet::default();
        for element in root
            .descendants()
            .filter(|element| element.name() == "style")
        {
            let kind = element.attribute("type").unwrap_or("text/css");
            if kind.is_empty() || kind.eq_ignore_ascii_case("text/css") {
                sheet.parse(&element.text());
            }
        }
        let mut dom = Dom {
            elements: Vec::new(),
            ids: HashMap::new(),
            queries: sheet.queries().to_vec(),
            scheme,
        };
        dom.adopt(&root, None, false)?;
        let (width, height) = viewport.unwrap_or_else(|| dom.intrinsic_size());
        let media = Media {
            width,
            height,
            scheme,
        };
        dom.cascade(&sheet, &sheet.active(&media))?;
        Ok(dom)
    }

    pub(crate) const fn root(&self) -> NodeId {
        NodeId(0)
    }

    pub(crate) fn element(&self, node: NodeId) -> &Element {
        &self.elements[node.0]
    }

    pub(crate) fn elements(&self) -> impl Iterator<Item = (NodeId, &Element)> {
        self.elements
            .iter()
            .enumerate()
            .map(|(index, element)| (NodeId(index), element))
    }

    pub(crate) fn attribute(&self, node: NodeId, name: &str) -> Option<&str> {
        self.elements[node.0].attribute(name)
    }

    pub(crate) fn by_id(&self, id: &str) -> Option<NodeId> {
        self.ids.get(id)?.last().copied()
    }

    pub(crate) fn find(&self, id: &str, tags: &[Tag]) -> Option<NodeId> {
        self.ids
            .get(id)?
            .iter()
            .rev()
            .copied()
            .find(|node| tags.contains(&self.elements[node.0].tag))
    }

    pub(crate) fn media_rules(&self) -> Option<MediaRules> {
        (!self.queries.is_empty()).then(|| MediaRules {
            queries: self.queries.clone(),
            scheme: self.scheme,
        })
    }

    pub(crate) fn intrinsic_size(&self) -> (f32, f32) {
        let root = self.root();
        let view = self.attribute(root, "viewBox").and_then(view_box);
        let side = |name: &str, fallback: f32| {
            self.attribute(root, name)
                .and_then(length)
                .filter(|given| given.unit == Unit::User && given.value > 0.0)
                .map_or(fallback, |given| given.value)
        };
        let width = side("width", view.map_or(FALLBACK_SIDE, |view| view.width));
        let height = side("height", view.map_or(FALLBACK_SIDE, |view| view.height));
        (width, height)
    }

    fn adopt(
        &mut self,
        source: &rikka_xml::Element<'_>,
        parent: Option<NodeId>,
        in_text: bool,
    ) -> Result<NodeId> {
        if self.elements.len() >= MAX_ELEMENTS {
            return Err(Error::TooComplex);
        }
        let foreign = source.namespace().is_some_and(|namespace| namespace != SVG);
        let tag = match foreign {
            true => Tag::Other,
            false => Tag::named(source.name(), in_text),
        };
        let node = NodeId(self.elements.len());
        let mut element = Element {
            tag,
            name: source.name().to_owned(),
            parent,
            children: Vec::new(),
            characters: String::new(),
            attributes: Vec::new(),
            previous: None,
            ordinal: 1,
            siblings: 1,
        };
        for (namespace, name, value) in source.attributes() {
            match (namespace, name) {
                (None, _) => element.set(name, value),
                (Some(XML), "space") => element.set("xml:space", value),
                (Some(_), "href") => {
                    if element.attribute("href").is_none() {
                        element.set("href", value);
                    }
                }
                (Some(_), _) => {}
            }
        }
        if let Some(id) = element.attribute("id") {
            self.ids.entry(id.to_owned()).or_default().push(node);
        }
        self.elements.push(element);
        if matches!(tag, Tag::Style | Tag::Other) {
            return Ok(node);
        }
        let mut children = Vec::new();
        let mut previous: Option<NodeId> = None;
        for content in source.contents() {
            let child = match content {
                rikka_xml::Content::Element(child) => {
                    let child = self.adopt(child, Some(node), tag.holds_text())?;
                    self.elements[child.0].previous = previous;
                    previous = Some(child);
                    child
                }
                rikka_xml::Content::Text(text) if tag.holds_text() => {
                    self.characters(node, text)?
                }
                rikka_xml::Content::Text(_) => continue,
            };
            children.push(child);
        }
        let elements: Vec<NodeId> = children
            .iter()
            .copied()
            .filter(|child| self.elements[child.0].tag != Tag::Characters)
            .collect();
        for (index, child) in elements.iter().enumerate() {
            let element = &mut self.elements[child.0];
            element.ordinal = index + 1;
            element.siblings = elements.len();
        }
        self.elements[node.0].children = children;
        Ok(node)
    }

    fn characters(&mut self, parent: NodeId, text: &str) -> Result<NodeId> {
        if self.elements.len() >= MAX_ELEMENTS {
            return Err(Error::TooComplex);
        }
        let node = NodeId(self.elements.len());
        self.elements.push(Element {
            tag: Tag::Characters,
            name: String::new(),
            parent: Some(parent),
            children: Vec::new(),
            characters: text.to_owned(),
            attributes: Vec::new(),
            previous: None,
            ordinal: 0,
            siblings: 0,
        });
        Ok(node)
    }

    fn cascade(&mut self, sheet: &Sheet, active: &[bool]) -> Result<()> {
        let work = Work::default();
        let mut styled: Vec<(usize, Vec<(String, String)>)> = Vec::new();
        for index in 0..self.elements.len() {
            let element = &self.elements[index];
            if element.tag == Tag::Characters {
                continue;
            }
            let mut found = match sheet.is_empty() {
                true => Vec::new(),
                false => sheet.matching(&*self, NodeId(index), active, &work),
            };
            if let Some(style) = element.attribute("style") {
                found.extend(declarations(style));
            }
            if !found.is_empty() {
                styled.push((index, found));
            }
        }
        if work.exhausted() {
            return Err(Error::TooComplex);
        }
        for (index, declarations) in styled {
            let element = &mut self.elements[index];
            let tag = element.tag;
            for (name, value) in declarations {
                let names: &[&str] = match name.as_str() {
                    "marker" => &MARKERS,
                    other => &[other][..],
                };
                for name in names {
                    if applies(name, tag) {
                        element.set(name, &value);
                    }
                }
            }
        }
        Ok(())
    }
}

fn applies(name: &str, tag: Tag) -> bool {
    name.starts_with("--")
        || (PRESENTATION.contains(&name) && (!GEOMETRY.contains(&name) || tag == Tag::Svg))
}

impl Scope for Dom {
    type Node = NodeId;

    fn parent(&self, node: NodeId) -> Option<NodeId> {
        self.elements[node.0].parent
    }

    fn previous(&self, node: NodeId) -> Option<NodeId> {
        self.elements[node.0].previous
    }

    fn tag(&self, node: NodeId) -> &str {
        &self.elements[node.0].name
    }

    fn attribute(&self, node: NodeId, name: &str) -> Option<&str> {
        self.elements[node.0].attribute(name)
    }

    fn ordinal(&self, node: NodeId) -> (usize, usize) {
        let element = &self.elements[node.0];
        (element.ordinal, element.siblings)
    }
}
