use rikka_vector::{Antialiasing, Path, Transform};

use crate::parse::dom::{NodeId, Tag};
use crate::parse::values::{Unit, length, reference, transform};
use crate::tree::build::{BLACK, Builder, antialiasing, paint_order, push, rule};
use crate::tree::glyphs::Glyphs;
use crate::tree::levels::Levels;
use crate::tree::shapes::outline;
use crate::tree::style::{Axis, Context};
use crate::tree::text::{
    Content, Decoration, DecorationLine, Flow, Run, Shift, Space, Typeset, anchor, baseline, font,
    lay_out, lines, lists, path_length, shift, space, text_length,
};
use crate::tree::{Fill, Ink, Node, Outline, Shape};

const CHARACTER_COST: usize = 512;

struct Painted<'p> {
    fill: &'p Option<Fill>,
    stroke: &'p Option<Outline>,
    stroke_first: bool,
}

fn painted(decoration: &Decoration) -> Painted<'_> {
    Painted {
        fill: &decoration.fill,
        stroke: &decoration.stroke,
        stroke_first: decoration.stroke_first,
    }
}

impl Builder<'_> {
    pub(super) fn text(&mut self, node: NodeId, out: &mut Vec<Node>) {
        let inherited = self.context().ancestry("xml:space").last().map_or(
            Space::Collapse,
            |value| match value.trim() {
                "preserve" => Space::Preserve,
                _ => Space::Collapse,
            },
        );
        let mut content = Content::default();
        self.collect(node, &mut content, None, true, inherited, &[]);
        content.finish(inherited);
        if self.exhausted || content.characters.iter().all(|character| !character.shown) {
            return;
        }
        let svg_glyphs = self.sources.svg_glyphs;
        let glyphs = self.glyphs.get_or_insert_with(|| Glyphs::new(svg_glyphs));
        let laid = lay_out(&content, glyphs);
        let parsed = std::mem::take(&mut glyphs.parsed_bytes);
        if glyphs.follows_scheme {
            self.sources.scheme_asked.set(true);
        }
        if !self.spend(parsed) {
            self.exhausted = true;
            return;
        }
        let mut children = Vec::new();
        for (run, typeset) in content.runs.iter().zip(laid.runs) {
            if !self.typeset(run, typeset, laid.bounds, &mut children) {
                self.exhausted = true;
                return;
            }
        }
        let Some(frame) = self.frame() else {
            return;
        };
        push(frame, children, out);
    }

    fn typeset(
        &mut self,
        run: &Run,
        typeset: Typeset,
        bounds: Option<rikka_types::Rect>,
        out: &mut Vec<Node>,
    ) -> bool {
        let Typeset {
            outlines,
            pictures,
            lines,
        } = typeset;
        let decorated: Vec<(&Decoration, Path)> = run.decorations.iter().zip(lines).collect();
        for wanted in [DecorationLine::Overline, DecorationLine::Underline] {
            for (decoration, path) in decorated.iter().filter(|(found, _)| found.line == wanted) {
                if !self.push_shape(path, &painted(decoration), bounds, run.antialiasing, out) {
                    return false;
                }
            }
        }
        let glyph_paint = Painted {
            fill: &run.fill,
            stroke: &run.stroke,
            stroke_first: run.stroke_first,
        };
        if !self.push_shape(&outlines, &glyph_paint, bounds, run.antialiasing, out) {
            return false;
        }
        for picture in pictures {
            if !self.spend(size_of::<Node>()) {
                return false;
            }
            out.push(Node::Glyph(picture));
        }
        for (decoration, path) in decorated
            .iter()
            .filter(|(found, _)| found.line == DecorationLine::LineThrough)
        {
            if !self.push_shape(path, &painted(decoration), bounds, run.antialiasing, out) {
                return false;
            }
        }
        true
    }

    fn push_shape(
        &mut self,
        path: &Path,
        painted: &Painted<'_>,
        bounds: Option<rikka_types::Rect>,
        antialiasing: Antialiasing,
        out: &mut Vec<Node>,
    ) -> bool {
        if path.is_empty() || path.is_truncated() {
            return true;
        }
        if !self.spend(size_of_val(path.points()) + size_of_val(path.verbs())) {
            return false;
        }
        out.push(Node::Shape(Shape {
            id: String::new(),
            transform: Transform::IDENTITY,
            path: path.clone(),
            bounds,
            fill: painted.fill.clone(),
            stroke: painted.stroke.clone(),
            stroke_first: painted.stroke_first,
            antialiasing,
        }));
        true
    }

    fn collect(
        &mut self,
        node: NodeId,
        content: &mut Content,
        flow: Option<usize>,
        shown: bool,
        space_mode: Space,
        shifts: &[Shift],
    ) {
        let deeper = Levels {
            depth: self.levels.depth + 1,
            ..self.levels
        };
        if !deeper.fits() {
            self.exhausted = true;
            return;
        }
        let context = self.context();
        let space_mode = space(&context, space_mode);
        let shown = shown && !context.is("display", "none") && self.passes_tests(node);
        let mut shifts = shifts.to_vec();
        shifts.extend(shift(&context));
        let dom = self.dom;
        let tag = dom.element(node).tag;
        let start = content.characters.len();
        let mut run = None;
        self.levels.depth += 1;
        for &child in &dom.element(node).children {
            match dom.element(child).tag {
                Tag::Characters => {
                    let run = *run.get_or_insert_with(|| self.run(content, &shifts));
                    for value in dom.element(child).characters.chars() {
                        if !self.spend(CHARACTER_COST) {
                            self.exhausted = true;
                            self.levels.depth -= 1;
                            return;
                        }
                        content.push(value, run, flow, shown, space_mode);
                    }
                }
                Tag::TextSpan => {
                    self.chain.push(child);
                    self.collect(child, content, flow, shown, space_mode, &shifts);
                    self.chain.pop();
                }
                Tag::TextPath if tag == Tag::Text => {
                    self.chain.push(child);
                    let path = self.flow(child, content);
                    self.collect(
                        child,
                        content,
                        path,
                        shown && path.is_some(),
                        space_mode,
                        &shifts,
                    );
                    self.chain.pop();
                }
                _ => {}
            }
        }
        self.levels.depth -= 1;
        let context = self.context();
        content
            .fits
            .extend(text_length(&context, start..content.characters.len()));
        if tag != Tag::TextPath {
            content.place(start, &lists(&context));
        }
    }

    fn run(&mut self, content: &mut Content, shifts: &[Shift]) -> usize {
        let context = self.context();
        let spacing = |name: &str| {
            context
                .property(name)
                .as_deref()
                .and_then(length)
                .map_or(0.0, |given| context.resolve(given, Axis::Horizontal))
        };
        let visible = context
            .property("visibility")
            .is_none_or(|value| value == "visible");
        let order = paint_order(&context);
        let mut run = Run {
            font: font(&context),
            size: context.font_size(),
            letter_spacing: spacing("letter-spacing"),
            word_spacing: spacing("word-spacing"),
            anchor: anchor(&context),
            baseline: baseline(&context),
            shifts: shifts.to_vec(),
            fill: None,
            stroke: None,
            stroke_first: order.stroke_first,
            decorations: Vec::new(),
            visible,
            antialiasing: antialiasing(&context, "text-rendering"),
        };
        match self.clipping {
            true => {
                run.fill = Some(self.clip_fill());
            }
            false => {
                run.fill = self.fill();
                run.stroke = self.outline();
            }
        }
        run.decorations = self.decorations();
        content.runs.push(run);
        content.runs.len() - 1
    }

    fn clip_fill(&self) -> Fill {
        Fill {
            ink: Ink::Color(BLACK),
            opacity: 1.0,
            rule: rule(&self.context(), "clip-rule"),
        }
    }

    fn decorations(&mut self) -> Vec<Decoration> {
        let declared: Vec<Vec<DecorationLine>> = (1..=self.chain.len())
            .map(|length| {
                lines(&Context {
                    dom: self.dom,
                    chain: &self.chain[..length],
                    viewport: self.viewport,
                })
            })
            .collect();
        let text = self
            .chain
            .iter()
            .rposition(|node| self.dom.element(*node).tag == Tag::Text)
            .unwrap_or(0);
        let mut decorations = Vec::new();
        for line in [
            DecorationLine::Underline,
            DecorationLine::Overline,
            DecorationLine::LineThrough,
        ] {
            if !declared.iter().any(|lines| lines.contains(&line)) {
                continue;
            }
            let owner = (text..self.chain.len())
                .rev()
                .find(|at| declared[*at].contains(&line))
                .unwrap_or(text);
            let inner = self.chain.split_off(owner + 1);
            let decoration = match self.clipping {
                true => Decoration {
                    line,
                    fill: Some(self.clip_fill()),
                    stroke: None,
                    stroke_first: false,
                },
                false => Decoration {
                    line,
                    fill: self.fill(),
                    stroke: self.outline(),
                    stroke_first: paint_order(&self.context()).stroke_first,
                },
            };
            self.chain.extend(inner);
            decorations.push(decoration);
        }
        decorations
    }

    fn flow(&mut self, node: NodeId, content: &mut Content) -> Option<usize> {
        let target = self
            .dom
            .attribute(node, "href")
            .and_then(reference)
            .and_then(|id| {
                self.dom.find(
                    id,
                    &[
                        Tag::Path,
                        Tag::Rect,
                        Tag::Circle,
                        Tag::Ellipse,
                        Tag::Line,
                        Tag::Polyline,
                        Tag::Polygon,
                    ],
                )
            })?;
        let chain = self.ancestry(target);
        let target_context = Context {
            dom: self.dom,
            chain: &chain,
            viewport: self.viewport,
        };
        let path = outline(self.dom.element(target).tag, &target_context)?;
        let placed = match target_context.attribute("transform") {
            Some(text) => transform(text).filter(Transform::is_finite)?,
            None => Transform::IDENTITY,
        };
        let path = path.transformed(&placed);
        let context = self.context();
        let start = match context.attribute("startOffset").and_then(length) {
            Some(given) if given.unit == Unit::Percent => path_length(&path) * given.value / 100.0,
            Some(given) => context.resolve(given, Axis::Horizontal),
            None => 0.0,
        };
        if !self.spend(size_of_val(path.points()) + size_of_val(path.verbs())) {
            self.exhausted = true;
            return None;
        }
        content.flows.push(Flow { path, start });
        Some(content.flows.len() - 1)
    }
}
