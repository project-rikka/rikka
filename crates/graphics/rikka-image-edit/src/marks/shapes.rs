use rikka_types::{Color, Point};

use crate::marks::brush::{BrushStroke, Ink};
use crate::marks::mark::{Mark, ellipse_points, rectangle_points};
use crate::marks::marking::Marking;
use crate::marks::outline::outlined;
use crate::marks::path::Subpath;
use crate::marks::path_mark::{FillRule, PathMark};
use crate::marks::tool::Tool;

const HEAD_WIDTH: f32 = 3.0;
const MIN_HEAD_WIDTH: f32 = 12.0;
const HEAD_LENGTH: f32 = 0.9;
const MAX_HEAD_SHARE: f32 = 0.6;

const CAPSULE: f32 = 0.0;
const BAR: f32 = 1.0;
const TRIANGLE: f32 = 2.0;
const BOX: f32 = 3.0;
const OVAL: f32 = 4.0;
const SEGMENT: f32 = 5.0;
const EDGE: f32 = 6.0;

const SOLID: u32 = 0;
const BRUSH: u32 = 1;
const AREA: u32 = 2;
const ERASE: u32 = 4;
const ALIASED: u32 = 8;
const EVEN_ODD: u32 = 16;

pub(crate) const STRIDE: usize = 12;
pub(crate) const TILE: u32 = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Erasure {
    Remove,
    Paint,
}

#[derive(Debug, Default)]
pub(crate) struct Shapes {
    pub(crate) shapes: Vec<f32>,
    pub(crate) inks: Vec<f32>,
    pub(crate) tiles: Vec<u32>,
    pub(crate) entries: Vec<u32>,
    pub(crate) across: u32,
    pub(crate) erases: bool,
}

impl Shapes {
    pub(crate) fn bounds(&self, extent: [u32; 2]) -> Option<[u32; 4]> {
        let mut bounds: Option<[u32; 4]> = None;
        for (index, span) in self.tiles.windows(2).enumerate() {
            if span[0] == span[1] {
                continue;
            }
            let index = index as u32;
            let [column, row] = [index % self.across, index / self.across];
            let [left, top] = [column * TILE, row * TILE];
            let [right, bottom] = [(left + TILE).min(extent[0]), (top + TILE).min(extent[1])];
            bounds = Some(match bounds {
                None => [left, top, right, bottom],
                Some([a, b, c, d]) => [a.min(left), b.min(top), c.max(right), d.max(bottom)],
            });
        }
        bounds.map(|[left, top, right, bottom]| [left, top, right - left, bottom - top])
    }
}

type Bins = ([u32; 2], [u32; 2]);

#[derive(Clone, Copy)]
enum SegmentEnd {
    Round,
    Square,
}

struct Builder {
    shapes: Vec<f32>,
    bins: Vec<Option<Bins>>,
    extent: [u32; 2],
    owner: u32,
}

impl Builder {
    fn capsule(&mut self, a: Point, b: Point, radius: f32) {
        self.push(CAPSULE, radius, [a, b, a]);
    }

    fn bar(&mut self, a: Point, b: Point, radius: f32) {
        self.push(BAR, radius, [a, b, a]);
    }

    fn triangle(&mut self, a: Point, b: Point, c: Point) {
        self.push(TRIANGLE, 0.0, [a, b, c]);
    }

    fn filled(&mut self, kind: f32, low: Point, high: Point) {
        self.push(kind, 0.0, [low, high, low]);
    }

    fn segment(&mut self, a: Point, b: Point, radius: f32) {
        self.push(SEGMENT, radius, [a, b, a]);
    }

    fn push(&mut self, kind: f32, radius: f32, points: [Point; 3]) {
        let at = self.shapes.len();
        self.record(kind, radius, points);
        let bins = shape_bins(&self.shapes[at..at + STRIDE], self.extent);
        self.bins.push(bins);
    }

    fn record(&mut self, kind: f32, radius: f32, points: [Point; 3]) {
        self.shapes.extend([kind, self.owner as f32, radius, 0.0]);
        for point in points {
            self.shapes.extend([point.x, point.y]);
        }
        self.shapes.extend([0.0, 0.0]);
    }

    fn polygons<'a>(&mut self, polygons: impl Iterator<Item = &'a [Point]> + Clone) {
        let right = polygons
            .clone()
            .flatten()
            .map(|point| point.x)
            .fold(f32::NEG_INFINITY, f32::max);
        for polygon in polygons {
            if polygon.len() < 2 {
                continue;
            }
            for (at, from) in polygon.iter().enumerate() {
                let to = polygon[(at + 1) % polygon.len()];
                if from.y == to.y {
                    continue;
                }
                self.record(EDGE, 0.0, [*from, to, *from]);
                let bins = edge_bins(*from, to, right, self.extent);
                self.bins.push(bins);
            }
        }
    }

    fn chain(&mut self, points: &[Point], radius: f32, end: SegmentEnd) {
        match points {
            [] => {}
            [only] => self.capsule(*only, *only, radius),
            _ => {
                for pair in points.windows(2) {
                    match end {
                        SegmentEnd::Square => self.bar(pair[0], pair[1], radius),
                        SegmentEnd::Round => self.capsule(pair[0], pair[1], radius),
                    }
                }
            }
        }
    }
}

struct Inks<F> {
    values: Vec<f32>,
    translate: F,
    erasure: Erasure,
    erases: bool,
}

impl<F: Fn(Color) -> [f32; 4]> Inks<F> {
    fn solid(&mut self, color: Color) {
        self.push(self.translated(color), SOLID, [0.0; 3]);
    }

    fn area(&mut self, color: Color, rule: FillRule, antialiased: bool) {
        let mut mode = AREA | aliased(antialiased);
        if rule == FillRule::EvenOdd {
            mode |= EVEN_ODD;
        }
        self.push(self.translated(color), mode, [0.0; 3]);
    }

    fn brush(&mut self, stroke: &BrushStroke) {
        let brush = stroke.brush().clamped();
        let mode = BRUSH | aliased(brush.antialiased);
        let shape = [brush.hardness, brush.flow, brush.size / 2.0];
        let (mut ink, mode) = match (stroke.ink(), self.erasure) {
            (Ink::Paint(color), _) | (Ink::Erase { background: color }, Erasure::Paint) => {
                (self.translated(color), mode)
            }
            (Ink::Erase { .. }, Erasure::Remove) => {
                self.erases = true;
                ([0.0, 0.0, 0.0, 1.0], mode | ERASE)
            }
        };
        ink[3] *= brush.opacity;
        self.push(ink, mode, shape);
    }

    fn translated(&self, color: Color) -> [f32; 4] {
        (self.translate)(color)
    }

    fn push(&mut self, ink: [f32; 4], mode: u32, shape: [f32; 3]) {
        self.values.extend(ink);
        self.values.push(mode as f32);
        self.values.extend(shape);
    }
}

fn aliased(antialiased: bool) -> u32 {
    match antialiased {
        true => 0,
        false => ALIASED,
    }
}

pub(crate) fn bin_shapes<'a>(
    markings: impl IntoIterator<Item = &'a Marking>,
    scale: f32,
    extent: [u32; 2],
    erasure: Erasure,
    ink: impl Fn(Color) -> [f32; 4],
) -> Shapes {
    let mut builder = Builder {
        shapes: Vec::new(),
        bins: Vec::new(),
        extent,
        owner: 0,
    };
    let mut inks = Inks {
        values: Vec::new(),
        translate: ink,
        erasure,
        erases: false,
    };
    for marking in markings {
        match marking.scaled(scale) {
            Marking::Mark(mark) => push_mark(&mut builder, &mut inks, &mark),
            Marking::Brush(stroke) => {
                push_brush_stroke(&mut builder, &stroke);
                inks.brush(&stroke);
                builder.owner += 1;
            }
            Marking::Path(path_mark) => push_path_mark(&mut builder, &mut inks, &path_mark),
        }
    }
    let across = extent[0].div_ceil(TILE);
    let down = extent[1].div_ceil(TILE);
    let count = (across * down) as usize;
    let mut lists: Vec<Vec<u32>> = vec![Vec::new(); count];
    for (shape, bins) in builder.bins.iter().enumerate() {
        let Some((low, high)) = bins else {
            continue;
        };
        for row in low[1]..=high[1] {
            for column in low[0]..=high[0] {
                lists[(row * across + column) as usize].push(shape as u32);
            }
        }
    }
    let mut tiles = Vec::with_capacity(count + 1);
    let mut entries = Vec::new();
    tiles.push(0);
    for list in &lists {
        entries.extend_from_slice(list);
        tiles.push(entries.len() as u32);
    }
    if entries.is_empty() {
        entries.push(0);
    }
    Shapes {
        shapes: builder.shapes,
        inks: inks.values,
        tiles,
        entries,
        across,
        erases: inks.erases,
    }
}

fn push_mark<F: Fn(Color) -> [f32; 4]>(builder: &mut Builder, inks: &mut Inks<F>, mark: &Mark) {
    if let Some(fill) = mark.fill() {
        push_fill(builder, mark);
        inks.solid(fill);
        builder.owner += 1;
    }
    if mark.outlines() {
        push_outline(builder, mark);
        inks.solid(mark.color());
        builder.owner += 1;
    }
}

fn push_brush_stroke(builder: &mut Builder, stroke: &BrushStroke) {
    let radius = stroke.brush().clamped().size / 2.0;
    let path = smoothed(stroke.points());
    match path.as_slice() {
        [] => {}
        [only] => builder.segment(*only, *only, radius),
        _ => {
            for pair in path.windows(2) {
                builder.segment(pair[0], pair[1], radius);
            }
        }
    }
}

fn push_path_mark<F: Fn(Color) -> [f32; 4]>(
    builder: &mut Builder,
    inks: &mut Inks<F>,
    path_mark: &PathMark,
) {
    if let Some((color, rule)) = path_mark.fill() {
        builder.polygons(path_mark.path().subpaths().iter().map(Subpath::points));
        inks.area(color, rule, path_mark.is_antialiased());
        builder.owner += 1;
    }
    if let Some((color, style)) = path_mark.stroke() {
        let outline = outlined(path_mark.path(), style);
        builder.polygons(outline.iter().map(Vec::as_slice));
        inks.area(color, FillRule::NonZero, path_mark.is_antialiased());
        builder.owner += 1;
    }
}

fn shape_bins(data: &[f32], extent: [u32; 2]) -> Option<Bins> {
    let radius = data[2] + 1.5;
    let points = [(data[4], data[5]), (data[6], data[7]), (data[8], data[9])];
    let low_x = points.iter().map(|p| p.0).fold(f32::INFINITY, f32::min) - radius;
    let low_y = points.iter().map(|p| p.1).fold(f32::INFINITY, f32::min) - radius;
    let high_x = points.iter().map(|p| p.0).fold(f32::NEG_INFINITY, f32::max) + radius;
    let high_y = points.iter().map(|p| p.1).fold(f32::NEG_INFINITY, f32::max) + radius;
    let (width, height) = (extent[0] as f32, extent[1] as f32);
    if high_x < 0.0 || high_y < 0.0 || low_x >= width || low_y >= height {
        return None;
    }
    let (across, down) = (extent[0].div_ceil(TILE), extent[1].div_ceil(TILE));
    Some((
        [tile_index(low_x, across), tile_index(low_y, down)],
        [tile_index(high_x, across), tile_index(high_y, down)],
    ))
}

fn tile_index(position: f32, count: u32) -> u32 {
    ((position.max(0.0) / TILE as f32) as u32).min(count - 1)
}

fn edge_bins(from: Point, to: Point, right: f32, extent: [u32; 2]) -> Option<Bins> {
    let (top, bottom) = (from.y.min(to.y), from.y.max(to.y));
    let left = from.x.min(to.x);
    let (width, height) = (extent[0] as f32, extent[1] as f32);
    if bottom < 0.0 || top >= height || left >= width {
        return None;
    }
    let (across, down) = (extent[0].div_ceil(TILE), extent[1].div_ceil(TILE));
    Some((
        [tile_index(left, across), tile_index(top, down)],
        [
            tile_index(right.max(left), across),
            tile_index(bottom, down),
        ],
    ))
}

fn push_outline(builder: &mut Builder, mark: &Mark) {
    let radius = mark.thickness() / 2.0;
    if let Some(dot) = mark.dot() {
        builder.capsule(dot, dot, radius);
        return;
    }
    let (from, to) = mark.endpoints();
    match mark.tool() {
        Tool::Pen => builder.chain(&smoothed(mark.points()), radius, SegmentEnd::Round),
        Tool::Highlighter => builder.chain(&smoothed(mark.points()), radius, SegmentEnd::Square),
        Tool::Line => builder.capsule(from, to, radius),
        Tool::Arrow => arrow(builder, from, to, mark.thickness()),
        Tool::Rectangle => {
            builder.chain(&rectangle_points(mark.span()), radius, SegmentEnd::Round);
        }
        Tool::Ellipse => {
            let span = mark.span();
            let (radius_x, radius_y) = (span.width() / 2.0, span.height() / 2.0);
            let perimeter =
                std::f32::consts::TAU * ((radius_x * radius_x + radius_y * radius_y) / 2.0).sqrt();
            let steps = ((perimeter / 3.0).ceil() as usize).clamp(24, 512);
            builder.chain(&ellipse_points(span, steps), radius, SegmentEnd::Round);
        }
    }
}

fn push_fill(builder: &mut Builder, mark: &Mark) {
    let span = mark.span();
    let (low, high) = (
        Point::new(span.min_x(), span.min_y()),
        Point::new(span.max_x(), span.max_y()),
    );
    match mark.tool() {
        Tool::Rectangle => builder.filled(BOX, low, high),
        Tool::Ellipse => builder.filled(OVAL, low, high),
        Tool::Pen | Tool::Highlighter | Tool::Line | Tool::Arrow => {}
    }
}

fn arrow(builder: &mut Builder, from: Point, to: Point, thickness: f32) {
    let length = from.distance(to);
    if length <= f32::EPSILON {
        builder.capsule(from, to, thickness / 2.0);
        return;
    }
    let along = Point::new((to.x - from.x) / length, (to.y - from.y) / length);
    let mut width = (thickness * HEAD_WIDTH).max(MIN_HEAD_WIDTH);
    let mut head = width * HEAD_LENGTH;
    let room = length * MAX_HEAD_SHARE;
    if head > room {
        width *= room / head;
        head = room;
    }
    let shaft = (thickness / 2.0).min(width / HEAD_WIDTH / 2.0);
    let base = Point::new(to.x - along.x * head, to.y - along.y * head);
    let start = Point::new(from.x + along.x * shaft, from.y + along.y * shaft);
    if (base.x - start.x) * along.x + (base.y - start.y) * along.y > 0.0 {
        builder.bar(start, base, shaft);
    }
    let across = Point::new(-along.y * width / 2.0, along.x * width / 2.0);
    builder.triangle(
        to,
        Point::new(base.x + across.x, base.y + across.y),
        Point::new(base.x - across.x, base.y - across.y),
    );
}

fn smoothed(points: &[Point]) -> Vec<Point> {
    let (Some(&first), Some(&last)) = (points.first(), points.last()) else {
        return Vec::new();
    };
    if points.len() <= 2 {
        return vec![first, last];
    }
    let mut start = midpoint(first, points[1]);
    let mut curve = vec![first, start];
    for pair in points[1..].windows(2) {
        let control = pair[0];
        let end = midpoint(pair[0], pair[1]);
        let length = start.distance(control) + control.distance(end);
        let steps = ((length / 2.0).ceil() as usize).clamp(1, 16);
        for step in 1..=steps {
            let t = step as f32 / steps as f32;
            let u = 1.0 - t;
            curve.push(Point::new(
                u * u * start.x + 2.0 * u * t * control.x + t * t * end.x,
                u * u * start.y + 2.0 * u * t * control.y + t * t * end.y,
            ));
        }
        start = end;
    }
    curve.push(last);
    curve
}

fn midpoint(from: Point, to: Point) -> Point {
    Point::new((from.x + to.x) / 2.0, (from.y + to.y) / 2.0)
}
