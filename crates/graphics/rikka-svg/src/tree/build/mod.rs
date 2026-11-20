mod filtering;
mod marking;
mod typesetting;

use std::collections::HashMap;
use std::sync::Arc;

use rikka_types::{Color, Point, Rect};
use rikka_vector::{Antialiasing, BlendMode, Cap, FillRule, Join, Path, Stroke, Transform};

use crate::error::{Error, Result};
use crate::parse::colors::color;
use crate::parse::dom::{Dom, NodeId, Tag};
use crate::parse::values::{
    Length, Unit, after_reference, aspect_ratio, fit, length, lengths, reference, transform,
    view_box,
};
use crate::render::bounds::content_bounds;
use crate::resources::Sources;
use crate::tree::filters::{Filter, blend_mode};
use crate::tree::glyphs::Glyphs;
use crate::tree::images::picture;
use crate::tree::levels::Levels;
use crate::tree::servers::{Painted, gradient, measure, tile, units};
use crate::tree::shapes::outline;
use crate::tree::style::{Axis, Context, Viewport};
use crate::tree::{
    Clip, Fill, Group, Ink, Masking, Node, Outline, Pattern, Picture, PictureContent, Shape,
    StrokeSpace, Tree, Units,
};

const FALLBACK_SIDE: f32 = 100.0;
const MAX_USE_DEPTH: usize = 32;
const FILTERED_LAYER_COST: usize = 9;
const BLACK: Color = Color::rgba(0.0, 0.0, 0.0, 1.0);

pub(crate) fn build(dom: &Dom, sources: &Sources<'_>) -> Result<Tree> {
    let root = dom.root();
    let view = dom.attribute(root, "viewBox").and_then(view_box);
    let side = |name: &str| dom.attribute(root, name).and_then(length);
    let fallback = view.map_or((FALLBACK_SIDE, FALLBACK_SIDE), |view| {
        (view.width, view.height)
    });
    let fixed = |side: Option<Length>, whole: f32| match side {
        Some(Length {
            value,
            unit: Unit::Percent,
        }) => Some(whole * value / 100.0),
        Some(Length {
            value,
            unit: Unit::User,
        }) => Some(value),
        Some(Length { value, .. }) => Some(value * 16.0),
        None => None,
    };
    let (given_width, given_height) = (side("width"), side("height"));
    let mut width = fixed(given_width, fallback.0);
    let mut height = fixed(given_height, fallback.1);
    if let Some(view) = view {
        match (width, height) {
            (Some(width), None) => height = Some(width * view.height / view.width),
            (None, Some(height)) => width = Some(height * view.width / view.height),
            _ => {}
        }
    }
    let sized = view.is_some()
        || [given_width, given_height]
            .iter()
            .all(|side| side.is_some_and(|side| side.unit != Unit::Percent));
    let (mut width, mut height) = (width.unwrap_or(fallback.0), height.unwrap_or(fallback.1));
    if !(width > 0.0 && height > 0.0) || !(width + height).is_finite() {
        return Err(Error::NoSize);
    }
    let mut builder = Builder {
        dom,
        sources,
        chain: vec![root],
        viewport: Viewport {
            width: view.map_or(width, |view| view.width),
            height: view.map_or(height, |view| view.height),
        },
        gradients: HashMap::new(),
        patterns: HashMap::new(),
        clips: HashMap::new(),
        masks: HashMap::new(),
        filters: HashMap::new(),
        building: Vec::new(),
        pictures: HashMap::new(),
        context_paint: None,
        glyphs: None,
        uses: 0,
        levels: Levels::default(),
        reach: Levels::default(),
        exhausted: false,
        clipping: false,
    };
    let mut children = Vec::new();
    builder.children(root, &mut children);
    if builder.exhausted {
        return Err(Error::TooComplex);
    }
    let mut group = Group {
        children,
        ..Group::default()
    };
    if !sized
        && let Some(bounds) = content_bounds(&group, &Transform::IDENTITY)
        && bounds.max_x() > 0.0
        && bounds.max_y() > 0.0
    {
        (width, height) = (bounds.max_x(), bounds.max_y());
    }
    if let Some(view) = view {
        let aspect = dom
            .attribute(root, "preserveAspectRatio")
            .map(aspect_ratio)
            .unwrap_or_default();
        group.transform = fit(view, aspect, width, height);
    }
    Ok(Tree {
        width,
        height,
        root: group,
    })
}

struct Builder<'b> {
    dom: &'b Dom,
    sources: &'b Sources<'b>,
    chain: Vec<NodeId>,
    viewport: Viewport,
    gradients: HashMap<NodeId, Option<Ink>>,
    patterns: HashMap<NodeId, Cached<Pattern>>,
    clips: HashMap<NodeId, Cached<Clip>>,
    masks: HashMap<NodeId, Cached<Masking>>,
    filters: HashMap<NodeId, Cached<Filter>>,
    building: Vec<NodeId>,
    pictures: HashMap<NodeId, Option<PictureContent>>,
    context_paint: Option<ContextPaint>,
    glyphs: Option<Glyphs>,
    uses: usize,
    levels: Levels,
    reach: Levels,
    exhausted: bool,
    clipping: bool,
}

struct Cached<T> {
    built: Option<Arc<T>>,
    extent: Levels,
}

#[derive(Clone)]
struct ContextPaint {
    fill: Option<Ink>,
    stroke: Option<Ink>,
}

struct Frame {
    group: Group,
    plain: bool,
}

impl<'b> Builder<'b> {
    fn context(&self) -> Context<'_> {
        Context {
            dom: self.dom,
            chain: &self.chain,
            viewport: self.viewport,
        }
    }

    fn children(&mut self, node: NodeId, out: &mut Vec<Node>) {
        for &child in &self.dom.element(node).children {
            self.element(child, out);
        }
    }

    fn element(&mut self, node: NodeId, out: &mut Vec<Node>) {
        let tag = self.dom.element(node).tag;
        let drawn = match self.clipping {
            true => matches!(
                tag,
                Tag::Use
                    | Tag::Path
                    | Tag::Rect
                    | Tag::Circle
                    | Tag::Ellipse
                    | Tag::Line
                    | Tag::Polyline
                    | Tag::Polygon
                    | Tag::Text
            ),
            false => matches!(
                tag,
                Tag::Svg
                    | Tag::Group
                    | Tag::Use
                    | Tag::Switch
                    | Tag::Path
                    | Tag::Rect
                    | Tag::Circle
                    | Tag::Ellipse
                    | Tag::Line
                    | Tag::Polyline
                    | Tag::Polygon
                    | Tag::Image
                    | Tag::Text
            ),
        };
        if !drawn {
            return;
        }
        let deeper = Levels {
            depth: self.levels.depth + 1,
            ..self.levels
        };
        if !deeper.fits() || !self.spend(size_of::<Node>()) {
            self.exhausted = true;
            return;
        }
        self.enter(deeper);
        self.chain.push(node);
        self.entered(node, tag, out);
        self.chain.pop();
        self.levels.depth -= 1;
    }

    fn spend(&self, bytes: usize) -> bool {
        self.sources.budget.spend(bytes)
    }

    fn enter(&mut self, levels: Levels) {
        self.levels = levels;
        self.reach = self.reach.highest(levels);
    }

    fn layered(&mut self, frame: &Frame, build: impl FnOnce(&mut Self)) {
        let cost = match (frame.plain, frame.group.filters.is_empty()) {
            (true, _) => 0,
            (false, true) => 1,
            (false, false) => FILTERED_LAYER_COST,
        };
        self.within_layers(cost, build);
    }

    fn within_layers(&mut self, cost: usize, build: impl FnOnce(&mut Self)) {
        let layered = Levels {
            layers: self.levels.layers + cost,
            ..self.levels
        };
        if !layered.fits() {
            self.exhausted = true;
            return;
        }
        self.enter(layered);
        build(self);
        self.levels.layers -= cost;
    }

    fn resource<T>(
        &mut self,
        node: NodeId,
        cache: fn(&mut Self) -> &mut HashMap<NodeId, Cached<T>>,
        build: impl FnOnce(&mut Self) -> Option<Arc<T>>,
    ) -> Option<Arc<T>> {
        let start = self.levels;
        if let Some(cached) = cache(self).get(&node) {
            let (built, reached) = (cached.built.clone(), start.plus(cached.extent));
            if !reached.fits() {
                return None;
            }
            self.reach = self.reach.highest(reached);
            return built;
        }
        let nested = Levels {
            nesting: start.nesting + 1,
            ..start
        };
        if self.building.contains(&node) || !nested.fits() {
            return None;
        }
        let outer = std::mem::replace(&mut self.reach, start);
        self.building.push(node);
        self.enter(nested);
        let built = build(self);
        self.building.pop();
        self.levels = start;
        let extent = self.reach.above(start);
        self.reach = outer.highest(self.reach);
        cache(self).insert(
            node,
            Cached {
                built: built.clone(),
                extent,
            },
        );
        built
    }

    fn entered(&mut self, node: NodeId, tag: Tag, out: &mut Vec<Node>) {
        if self.context().is("display", "none") || !self.passes_tests(node) {
            return;
        }
        match tag {
            Tag::Group => {
                let Some(frame) = self.frame() else { return };
                let mut children = Vec::new();
                self.layered(&frame, |builder| builder.children(node, &mut children));
                push(frame, children, out);
            }
            Tag::Switch => {
                let Some(frame) = self.frame() else { return };
                let mut children = Vec::new();
                let chosen = self
                    .dom
                    .element(node)
                    .children
                    .iter()
                    .copied()
                    .find(|child| {
                        self.dom.element(*child).tag != Tag::Other && self.passes_tests(*child)
                    });
                if let Some(chosen) = chosen {
                    self.layered(&frame, |builder| builder.element(chosen, &mut children));
                }
                push(frame, children, out);
            }
            Tag::Svg => {
                let Some(frame) = self.frame() else { return };
                let mut children = Vec::new();
                self.layered(&frame, |builder| {
                    builder.viewport_content(node, node, &mut children);
                });
                push(frame, children, out);
            }
            Tag::Use => self.instance(node, out),
            Tag::Image => self.image(node, out),
            Tag::Text => self.text(node, out),
            _ => self.shape(tag, out),
        }
    }

    fn passes_tests(&self, node: NodeId) -> bool {
        let attribute = |name: &str| self.dom.attribute(node, name);
        if attribute("requiredExtensions").is_some_and(|value| !value.trim().is_empty()) {
            return false;
        }
        attribute("systemLanguage").is_none_or(|languages| {
            languages.split(',').any(|language| {
                let language = language.trim();
                language == "en" || language.starts_with("en-")
            })
        })
    }

    fn frame(&mut self) -> Option<Frame> {
        let context = self.context();
        let mut placed = match context.attribute("transform") {
            Some(text) => transform(text).filter(Transform::is_finite)?,
            None => Transform::IDENTITY,
        };
        if placed.determinant() == 0.0 {
            return None;
        }
        if let Some(origin) = transform_origin(&context).filter(|_| !placed.is_identity()) {
            placed = Transform::translate(-origin.x, -origin.y)
                .then(placed)
                .then(Transform::translate(origin.x, origin.y));
        }
        let mut group = Group {
            id: context.attribute("id").unwrap_or_default().to_owned(),
            transform: placed,
            ..Group::default()
        };
        let clip = context
            .property("clip-path")
            .map(|value| value.into_owned());
        if self.clipping {
            group.clip = clip.as_deref().and_then(|value| self.clip(value));
            let plain = group.clip.is_none();
            return Some(Frame { group, plain });
        }
        group.opacity = context.unit("opacity").unwrap_or(1.0);
        group.blend = context
            .property("mix-blend-mode")
            .map_or(BlendMode::SourceOver, |mode| blend_mode(&mode));
        group.isolated = context.is("isolation", "isolate");
        let mask = context.property("mask").map(|value| value.into_owned());
        let filters = context.property("filter").map(|value| value.into_owned());
        group.clip = clip.as_deref().and_then(|value| self.clip(value));
        group.mask = mask.as_deref().and_then(|value| self.masking(value));
        if let Some(value) = filters.as_deref().filter(|value| *value != "none") {
            group.filters = self.filter_list(value)?;
        }
        let plain = group.is_plain();
        Some(Frame { group, plain })
    }

    fn shape(&mut self, tag: Tag, out: &mut Vec<Node>) {
        let context = self.context();
        if context
            .property("visibility")
            .is_some_and(|value| value != "visible")
        {
            return;
        }
        let Some(path) = outline(tag, &context) else {
            return;
        };
        let held = size_of_val(path.points()) + size_of_val(path.verbs());
        if !self.spend(held) {
            self.exhausted = true;
            return;
        }
        let (fill, stroke) = match self.clipping {
            true => (
                Some(Fill {
                    ink: Ink::Color(BLACK),
                    opacity: 1.0,
                    rule: rule(&context, "clip-rule"),
                }),
                None,
            ),
            false => (self.fill(), self.outline()),
        };
        let marked =
            !self.clipping && matches!(tag, Tag::Path | Tag::Line | Tag::Polyline | Tag::Polygon);
        let order = paint_order(&self.context());
        let markers = match marked {
            true => self.markers(&path, fill.as_ref(), stroke.as_ref()),
            false => Vec::new(),
        };
        let unpainted = fill.is_none() && stroke.is_none() && markers.is_empty();
        if unpainted
            && self
                .context()
                .property("filter")
                .is_none_or(|value| value == "none")
        {
            return;
        }
        let antialiasing = antialiasing(&self.context(), "shape-rendering");
        let Some(mut frame) = self.frame() else {
            return;
        };
        if unpainted && frame.group.filters.is_empty() {
            return;
        }
        let mut shape = Shape {
            id: String::new(),
            transform: Transform::IDENTITY,
            path,
            bounds: None,
            fill,
            stroke,
            stroke_first: order.stroke_first,
            antialiasing,
        };
        let folds = !self.clipping
            && markers.is_empty()
            && frame.group.opacity < 1.0
            && (shape.fill.is_none() || shape.stroke.is_none())
            && Group {
                opacity: 1.0,
                ..frame.group.clone()
            }
            .is_plain();
        if folds {
            if let Some(fill) = &mut shape.fill {
                fill.opacity *= frame.group.opacity;
            }
            if let Some(stroke) = &mut shape.stroke {
                stroke.opacity *= frame.group.opacity;
            }
            frame.group.opacity = 1.0;
            frame.plain = true;
        }
        if markers.is_empty() && frame.plain {
            shape.id = frame.group.id;
            shape.transform = frame.group.transform;
            out.push(Node::Shape(shape));
            return;
        }
        let painted = shape.fill.is_some() || shape.stroke.is_some();
        let markers = Node::Group(Group {
            children: markers,
            ..Group::default()
        });
        let children = &mut frame.group.children;
        match (order.markers, painted) {
            (_, false) => {
                children.push(Node::Shape(shape));
                children.push(markers);
            }
            (MarkerOrder::First, true) => {
                children.push(markers);
                children.push(Node::Shape(shape));
            }
            (MarkerOrder::Last, true) => {
                children.push(Node::Shape(shape));
                children.push(markers);
            }
            (MarkerOrder::Between, true) => {
                let (first, second) = split_paint(shape);
                children.extend(first.map(Node::Shape));
                children.push(markers);
                children.extend(second.map(Node::Shape));
            }
        }
        out.push(Node::Group(frame.group));
    }

    fn fill(&mut self) -> Option<Fill> {
        let ink = self.ink("fill", Some(BLACK))?;
        let context = self.context();
        Some(Fill {
            ink,
            opacity: context.unit("fill-opacity").unwrap_or(1.0),
            rule: rule(&context, "fill-rule"),
        })
    }

    fn outline(&mut self) -> Option<Outline> {
        let ink = self.ink("stroke", None)?;
        let context = self.context();
        let width = context
            .length("stroke-width", Axis::Diagonal)
            .unwrap_or(1.0);
        if width <= 0.0 || width.is_nan() {
            return None;
        }
        let dashes = context
            .property("stroke-dasharray")
            .filter(|value| value != "none")
            .and_then(|value| lengths(&value))
            .map(|dashes| {
                dashes
                    .into_iter()
                    .map(|dash| context.resolve(dash, Axis::Diagonal))
                    .collect::<Vec<f32>>()
            })
            .filter(|dashes| {
                dashes.iter().all(|dash| *dash >= 0.0) && dashes.iter().sum::<f32>() > 0.0
            })
            .unwrap_or_default();
        Some(Outline {
            ink,
            opacity: context.unit("stroke-opacity").unwrap_or(1.0),
            width,
            join: match context.property("stroke-linejoin").as_deref() {
                Some("round") => Join::Round,
                Some("bevel") => Join::Bevel,
                _ => Join::Miter,
            },
            cap: match context.property("stroke-linecap").as_deref() {
                Some("round") => Cap::Round,
                Some("square") => Cap::Square,
                _ => Cap::Butt,
            },
            miter_limit: context
                .number("stroke-miterlimit")
                .filter(|limit| *limit >= 1.0)
                .unwrap_or(Stroke::DEFAULT_MITER_LIMIT),
            dashes,
            dash_offset: context
                .length("stroke-dashoffset", Axis::Diagonal)
                .unwrap_or(0.0),
            space: match context.attribute("vector-effect").map(str::trim) {
                Some("non-scaling-stroke") => StrokeSpace::Host,
                _ => StrokeSpace::User,
            },
        })
    }

    fn ink(&mut self, name: &str, initial: Option<Color>) -> Option<Ink> {
        let context = self.context();
        let Some(value) = context.property(name) else {
            return initial.map(Ink::Color);
        };
        match (value.as_ref(), &self.context_paint) {
            ("context-fill", Some(paint)) => return paint.fill.clone(),
            ("context-stroke", Some(paint)) => return paint.stroke.clone(),
            _ => {}
        }
        let flat = |text: &str| match text {
            "none" => None,
            "currentColor" | "currentcolor" | "context-fill" | "context-stroke" => {
                Some(context.current_color())
            }
            other => color(other),
        };
        if !value.starts_with("url(") {
            return match value.as_ref() {
                "none" => None,
                other => flat(other).or(initial).map(Ink::Color),
            };
        }
        let fallback = after_reference(&value).and_then(flat);
        let server = reference(&value).and_then(|id| {
            self.dom.find(
                id,
                &[Tag::LinearGradient, Tag::RadialGradient, Tag::Pattern],
            )
        });
        let found = match server.map(|server| (server, self.dom.element(server).tag)) {
            Some((server, Tag::LinearGradient | Tag::RadialGradient)) => self.gradient(server),
            Some((server, Tag::Pattern)) => self.pattern(server).map(Ink::Pattern),
            _ => None,
        };
        found.or(fallback.map(Ink::Color))
    }

    fn gradient(&mut self, node: NodeId) -> Option<Ink> {
        if let Some(known) = self.gradients.get(&node) {
            return known.clone();
        }
        let ink = match gradient(self.dom, node, self.viewport) {
            Painted::Gradient(gradient) => Some(Ink::Gradient(Arc::new(gradient))),
            Painted::Flat(color) => Some(Ink::Color(color)),
            Painted::Nothing => None,
        };
        self.gradients.insert(node, ink.clone());
        ink
    }

    fn pattern(&mut self, node: NodeId) -> Option<Arc<Pattern>> {
        self.resource(
            node,
            |builder| &mut builder.patterns,
            |builder| {
                let tile = tile(builder.dom, node, builder.viewport)?;
                let content = builder.detached(tile.content, false);
                Some(Arc::new(Pattern {
                    units: tile.units,
                    content_units: tile.content_units,
                    transform: tile.transform,
                    rect: tile.rect,
                    view: tile.view,
                    content,
                }))
            },
        )
    }

    fn detached(&mut self, node: NodeId, clipping: bool) -> Group {
        let ancestry = self.ancestry(node);
        let chain = std::mem::replace(&mut self.chain, ancestry);
        let was_clipping = std::mem::replace(&mut self.clipping, clipping);
        let mut children = Vec::new();
        self.children(node, &mut children);
        self.chain = chain;
        self.clipping = was_clipping;
        Group {
            children,
            ..Group::default()
        }
    }

    fn clip(&mut self, value: &str) -> Option<Arc<Clip>> {
        let node = reference(value).and_then(|id| self.dom.find(id, &[Tag::ClipPath]))?;
        self.resource(
            node,
            |builder| &mut builder.clips,
            |builder| {
                let attribute = |name: &str| builder.dom.attribute(node, name);
                let placed = attribute("transform").map_or(Some(Transform::IDENTITY), transform)?;
                let clip_units = units(attribute("clipPathUnits"), Units::UserSpace);
                let nested = attribute("clip-path").map(str::to_owned);
                let nested = nested.as_deref().and_then(|value| builder.clip(value));
                let content = builder.detached(node, true);
                Some(Arc::new(Clip {
                    units: clip_units,
                    transform: placed,
                    clip: nested,
                    content,
                }))
            },
        )
    }

    fn masking(&mut self, value: &str) -> Option<Arc<Masking>> {
        let node = reference(value).and_then(|id| self.dom.find(id, &[Tag::Mask]))?;
        self.resource(
            node,
            |builder| &mut builder.masks,
            |builder| {
                let own = [node];
                let context = Context {
                    dom: builder.dom,
                    chain: &own,
                    viewport: builder.viewport,
                };
                let mask_units = units(context.attribute("maskUnits"), Units::BoundingBox);
                let side = |name: &str, default: f32, axis: Axis| {
                    measure(
                        context.attribute(name),
                        Length::percent(default),
                        mask_units,
                        axis,
                        &context,
                    )
                };
                let rect = Rect::new(
                    side("x", -10.0, Axis::Horizontal),
                    side("y", -10.0, Axis::Vertical),
                    side("width", 120.0, Axis::Horizontal),
                    side("height", 120.0, Axis::Vertical),
                );
                let content_units = units(context.attribute("maskContentUnits"), Units::UserSpace);
                let luminance = !context.is("mask-type", "alpha");
                let linear = context.is("color-interpolation", "linearRGB");
                let nested = context.attribute("mask").map(str::to_owned);
                let nested = nested.as_deref().and_then(|value| builder.masking(value));
                let content = builder.detached(node, false);
                (rect.width() > 0.0 && rect.height() > 0.0).then(|| {
                    Arc::new(Masking {
                        units: mask_units,
                        content_units,
                        rect,
                        luminance,
                        linear,
                        mask: nested,
                        content,
                    })
                })
            },
        )
    }

    fn picture(&mut self, node: NodeId) -> Option<PictureContent> {
        if let Some(known) = self.pictures.get(&node) {
            return known.clone();
        }
        let href = self.dom.attribute(node, "href");
        let content = match self.sources.prefetched {
            Some(prefetched) => href.and_then(|href| prefetched.get(href).cloned().flatten()),
            None => href.and_then(|href| picture(href, self.sources)),
        };
        self.pictures.insert(node, content.clone());
        content
    }

    fn ancestry(&self, node: NodeId) -> Vec<NodeId> {
        let mut ancestry = vec![node];
        let mut current = node;
        while let Some(parent) = self.dom.element(current).parent {
            ancestry.push(parent);
            current = parent;
        }
        ancestry.reverse();
        ancestry
    }

    fn instance(&mut self, node: NodeId, out: &mut Vec<Node>) {
        let target = self
            .dom
            .attribute(node, "href")
            .and_then(reference)
            .and_then(|id| self.dom.by_id(id));
        let Some(target) = target else { return };
        if self.chain.contains(&target) || self.uses >= MAX_USE_DEPTH {
            return;
        }
        let Some(mut frame) = self.frame() else {
            return;
        };
        let context = self.context();
        let offset = Transform::translate(
            context.coordinate("x", Axis::Horizontal).unwrap_or(0.0),
            context.coordinate("y", Axis::Vertical).unwrap_or(0.0),
        );
        frame.group.transform = offset.then(frame.group.transform);
        let tag = self.dom.element(target).tag;
        let mut children = Vec::new();
        self.uses += 1;
        self.layered(&frame, |builder| match tag {
            Tag::Symbol | Tag::Svg if !builder.clipping => {
                builder.chain.push(target);
                if !builder.context().is("display", "none") {
                    builder.viewport_content(target, node, &mut children);
                }
                builder.chain.pop();
            }
            Tag::Symbol | Tag::Svg => {}
            _ => builder.element(target, &mut children),
        });
        self.uses -= 1;
        push(frame, children, out);
    }

    fn viewport_content(&mut self, node: NodeId, sized_by: NodeId, out: &mut Vec<Node>) {
        let is_symbol = self.dom.element(node).tag == Tag::Symbol;
        let context = self.context();
        let whole = Length::percent(100.0);
        let side = |name: &str, axis: Axis| {
            let given = self
                .dom
                .attribute(sized_by, name)
                .or_else(|| self.dom.attribute(node, name))
                .and_then(length)
                .unwrap_or(whole);
            context.resolve(given, axis)
        };
        let (width, height) = (
            side("width", Axis::Horizontal),
            side("height", Axis::Vertical),
        );
        if !(width > 0.0 && height > 0.0) {
            return;
        }
        let (x, y) = match is_symbol || sized_by != node {
            true => (0.0, 0.0),
            false => (
                context.coordinate("x", Axis::Horizontal).unwrap_or(0.0),
                context.coordinate("y", Axis::Vertical).unwrap_or(0.0),
            ),
        };
        let view = context.attribute("viewBox").and_then(view_box);
        let aspect = context
            .attribute("preserveAspectRatio")
            .map(aspect_ratio)
            .unwrap_or_default();
        let overflows = matches!(
            context.property("overflow").as_deref(),
            Some("visible" | "auto")
        );
        let inner = view.map_or(Transform::IDENTITY, |view| fit(view, aspect, width, height));
        let outer = std::mem::replace(
            &mut self.viewport,
            Viewport {
                width: view.map_or(width, |view| view.width),
                height: view.map_or(height, |view| view.height),
            },
        );
        let mut children = Vec::new();
        self.within_layers(usize::from(!overflows), |builder| {
            builder.children(node, &mut children);
        });
        self.viewport = outer;
        if children.is_empty() {
            return;
        }
        let mut window = Path::new();
        window.add_rect(Rect::new(x, y, width, height));
        let clip = (!overflows).then(|| Arc::new(Clip::window(window)));
        out.push(Node::Group(Group {
            clip,
            children: vec![Node::Group(Group {
                transform: inner.then(Transform::translate(x, y)),
                children,
                ..Group::default()
            })],
            ..Group::default()
        }));
    }

    fn image(&mut self, node: NodeId, out: &mut Vec<Node>) {
        if self
            .context()
            .property("visibility")
            .is_some_and(|value| value != "visible")
        {
            return;
        }
        let Some(content) = self.picture(node) else {
            return;
        };
        let context = self.context();
        let natural = match &content {
            PictureContent::Raster(pixmap) => (pixmap.width() as f32, pixmap.height() as f32),
            PictureContent::Vector(tree) => (tree.width, tree.height),
        };
        let side = |name: &str, axis: Axis| {
            context
                .attribute(name)
                .filter(|value| *value != "auto")
                .and_then(|_| context.coordinate(name, axis))
        };
        let (width, height) = match (
            side("width", Axis::Horizontal),
            side("height", Axis::Vertical),
        ) {
            (Some(width), Some(height)) => (width, height),
            (Some(width), None) => (width, width * natural.1 / natural.0),
            (None, Some(height)) => (height * natural.0 / natural.1, height),
            (None, None) => natural,
        };
        if !(width > 0.0 && height > 0.0) {
            return;
        }
        let origin = Point::new(
            context.coordinate("x", Axis::Horizontal).unwrap_or(0.0),
            context.coordinate("y", Axis::Vertical).unwrap_or(0.0),
        );
        let smooth = !matches!(
            context.property("image-rendering").as_deref(),
            Some("optimizeSpeed" | "pixelated" | "crisp-edges")
        );
        let aspect = context
            .attribute("preserveAspectRatio")
            .map(aspect_ratio)
            .unwrap_or_default();
        let Some(mut frame) = self.frame() else {
            return;
        };
        let mut picture = Picture {
            id: String::new(),
            transform: Transform::IDENTITY,
            frame: Rect::new(origin.x, origin.y, width, height),
            aspect,
            smooth,
            content,
        };
        match frame.plain {
            true => {
                picture.id = frame.group.id;
                picture.transform = frame.group.transform;
                out.push(Node::Picture(picture));
            }
            false => {
                frame.group.children.push(Node::Picture(picture));
                out.push(Node::Group(frame.group));
            }
        }
    }
}

fn rule(context: &Context<'_>, name: &str) -> FillRule {
    match context.is(name, "evenodd") {
        true => FillRule::EvenOdd,
        false => FillRule::NonZero,
    }
}

fn transform_origin(context: &Context<'_>) -> Option<Point> {
    let value = context.property("transform-origin")?;
    let mut x = None;
    let mut y = None;
    let mut pending = Vec::new();
    for word in value.split_ascii_whitespace().take(2) {
        match word {
            "left" => x = Some(Length::percent(0.0)),
            "right" => x = Some(Length::percent(100.0)),
            "top" => y = Some(Length::percent(0.0)),
            "bottom" => y = Some(Length::percent(100.0)),
            "center" => pending.push(Length::percent(50.0)),
            other => pending.push(length(other)?),
        }
    }
    for given in pending {
        match x {
            None => x = Some(given),
            Some(_) => y = y.or(Some(given)),
        }
    }
    let center = Length::percent(50.0);
    Some(Point::new(
        context.resolve(x.unwrap_or(center), Axis::Horizontal),
        context.resolve(y.unwrap_or(center), Axis::Vertical),
    ))
}

fn antialiasing(context: &Context<'_>, name: &str) -> Antialiasing {
    match context.property(name).as_deref() {
        Some("crispEdges" | "optimizeSpeed") => Antialiasing::Disabled,
        _ => Antialiasing::Enabled,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MarkerOrder {
    First,
    Between,
    Last,
}

struct PaintOrder {
    stroke_first: bool,
    markers: MarkerOrder,
}

fn paint_order(context: &Context<'_>) -> PaintOrder {
    let value = context.property("paint-order");
    let mut named: Vec<&str> = value
        .as_deref()
        .unwrap_or_default()
        .split_ascii_whitespace()
        .filter(|word| matches!(*word, "fill" | "stroke" | "markers"))
        .collect();
    for missing in ["fill", "stroke", "markers"] {
        if !named.contains(&missing) {
            named.push(missing);
        }
    }
    let at = |name: &str| named.iter().position(|word| *word == name).unwrap_or(0);
    PaintOrder {
        stroke_first: at("stroke") < at("fill"),
        markers: match at("markers") {
            0 => MarkerOrder::First,
            1 => MarkerOrder::Between,
            _ => MarkerOrder::Last,
        },
    }
}

fn split_paint(shape: Shape) -> (Option<Shape>, Option<Shape>) {
    let fill_only = (shape.fill.is_some()).then(|| Shape {
        stroke: None,
        ..shape.clone()
    });
    let stroke_only = (shape.stroke.is_some()).then(|| Shape {
        fill: None,
        ..shape.clone()
    });
    match shape.stroke_first {
        true => (stroke_only, fill_only),
        false => (fill_only, stroke_only),
    }
}

fn push(frame: Frame, children: Vec<Node>, out: &mut Vec<Node>) {
    let mut group = frame.group;
    if children.is_empty() && group.filters.is_empty() {
        return;
    }
    if frame.plain && group.id.is_empty() && group.transform.is_identity() {
        out.extend(children);
        return;
    }
    group.children = children;
    out.push(Node::Group(group));
}
