use rikka_types::{Color, Point, Rect};

use crate::marks::style::ShapeStyle;
use crate::marks::tool::Tool;

const MIN_STEP: f32 = 1.5;

const MIN_MOVE: f32 = 0.5;

const MIN_SPAN: f32 = 3.0;

const OUTLINE_STEPS: usize = 48;

#[derive(Clone, Debug, PartialEq)]
pub struct Mark {
    tool: Tool,
    ink: Color,
    width: f32,
    style: ShapeStyle,
    points: Vec<Point>,
}

impl Mark {
    #[must_use]
    pub fn new(tool: Tool, ink: Color, width: f32, from: Point) -> Mark {
        Mark {
            tool,
            ink,
            width,
            style: ShapeStyle::Outline,
            points: vec![from],
        }
    }

    #[must_use]
    pub fn styled(mut self, style: ShapeStyle) -> Mark {
        if self.tool.fills() {
            self.style = style;
        }
        self
    }

    #[must_use]
    pub fn style(&self) -> ShapeStyle {
        self.style
    }

    pub fn extend_to(&mut self, at: Point) -> bool {
        if !self.tool.is_freehand() {
            if self.points.len() < 2 {
                self.points.push(at);
                return true;
            }
            let last = &mut self.points[1];
            if *last == at {
                return false;
            }
            *last = at;
            return true;
        }
        if self.last_point().distance(at) < MIN_STEP {
            return false;
        }
        self.points.push(at);
        true
    }

    fn last_point(&self) -> Point {
        self.points[self.points.len() - 1]
    }

    #[must_use]
    pub fn is_significant(&self) -> bool {
        if self.tool.is_freehand() {
            return !self.points.is_empty();
        }
        let span = self.span();
        span.width().max(span.height()) >= MIN_SPAN
    }

    #[must_use]
    pub fn tool(&self) -> Tool {
        self.tool
    }

    #[must_use]
    pub(crate) fn color(&self) -> Color {
        self.ink.scale_alpha(self.tool.opacity())
    }

    #[must_use]
    pub(crate) fn fill(&self) -> Option<Color> {
        self.style.fill(self.ink)
    }

    #[must_use]
    pub(crate) fn outlines(&self) -> bool {
        self.style.outlines()
    }

    #[must_use]
    pub(crate) fn thickness(&self) -> f32 {
        self.tool.thickness(self.width)
    }

    #[must_use]
    pub(crate) fn points(&self) -> &[Point] {
        &self.points
    }

    #[must_use]
    pub(crate) fn span(&self) -> Rect {
        let (from, to) = (self.points[0], self.last_point());
        Rect::new(
            from.x.min(to.x),
            from.y.min(to.y),
            (to.x - from.x).abs(),
            (to.y - from.y).abs(),
        )
    }

    #[must_use]
    pub(crate) fn endpoints(&self) -> (Point, Point) {
        (self.points[0], self.last_point())
    }

    #[must_use]
    pub(crate) fn dot(&self) -> Option<Point> {
        let first = self.points[0];
        let still = self
            .points
            .iter()
            .all(|point| point.distance(first) < MIN_MOVE);
        (self.tool.is_freehand() && still).then_some(first)
    }

    #[must_use]
    pub(crate) fn scaled(&self, factor: f32) -> Mark {
        Mark {
            tool: self.tool,
            ink: self.ink,
            width: self.width * factor,
            style: self.style,
            points: self
                .points
                .iter()
                .map(|point| Point::new(point.x * factor, point.y * factor))
                .collect(),
        }
    }

    pub(crate) fn touches(&self, at: Point, reach: f32) -> bool {
        if self.fill().is_some() && self.encloses(at, reach) {
            return true;
        }
        let tolerance = reach + self.thickness() / 2.0;
        let outline = self.outline();
        match outline.split_first() {
            None => false,
            Some((only, [])) => only.distance(at) <= tolerance,
            Some(_) => outline
                .windows(2)
                .any(|pair| distance_to_segment(pair[0], pair[1], at) <= tolerance),
        }
    }

    fn encloses(&self, at: Point, reach: f32) -> bool {
        let span = self.span();
        match self.tool {
            Tool::Rectangle => span.inflate(reach).contains(at),
            Tool::Ellipse => {
                let center = span.center();
                let across = span.width() / 2.0 + reach;
                let down = span.height() / 2.0 + reach;
                if across <= 0.0 || down <= 0.0 {
                    return false;
                }
                let (x, y) = ((at.x - center.x) / across, (at.y - center.y) / down);
                x * x + y * y <= 1.0
            }
            Tool::Pen | Tool::Highlighter | Tool::Line | Tool::Arrow => false,
        }
    }

    #[must_use]
    fn outline(&self) -> Vec<Point> {
        let span = self.span();
        match self.tool {
            Tool::Rectangle => rectangle_points(span),
            Tool::Ellipse => ellipse_points(span, OUTLINE_STEPS),
            Tool::Pen | Tool::Highlighter | Tool::Line | Tool::Arrow => self.points.clone(),
        }
    }
}

pub(crate) fn rectangle_points(span: Rect) -> Vec<Point> {
    vec![
        Point::new(span.min_x(), span.min_y()),
        Point::new(span.max_x(), span.min_y()),
        Point::new(span.max_x(), span.max_y()),
        Point::new(span.min_x(), span.max_y()),
        Point::new(span.min_x(), span.min_y()),
    ]
}

pub(crate) fn ellipse_points(span: Rect, steps: usize) -> Vec<Point> {
    let center = span.center();
    let (radius_x, radius_y) = (span.width() / 2.0, span.height() / 2.0);
    (0..=steps)
        .map(|step| {
            let angle = step as f32 / steps as f32 * std::f32::consts::TAU;
            Point::new(
                center.x + radius_x * angle.cos(),
                center.y + radius_y * angle.sin(),
            )
        })
        .collect()
}

fn distance_to_segment(from: Point, to: Point, at: Point) -> f32 {
    let direction = Point::new(to.x - from.x, to.y - from.y);
    let length_squared = direction.x * direction.x + direction.y * direction.y;
    if length_squared <= f32::EPSILON {
        return from.distance(at);
    }
    let t = (((at.x - from.x) * direction.x + (at.y - from.y) * direction.y) / length_squared)
        .clamp(0.0, 1.0);
    Point::new(from.x + direction.x * t, from.y + direction.y * t).distance(at)
}
