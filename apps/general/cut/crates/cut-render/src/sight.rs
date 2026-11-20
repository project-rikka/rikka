use cut_project::{Format, Homography, Projection};
use rikka_figure::Affine;
use rikka_image_edit::{FillRule, Mask, Path};
use rikka_types::{Point, Rect};

const SLACK: f32 = 4.0;

pub(crate) enum Cover {
    Whole,
    Within(Mask),
    Nowhere,
}

pub(crate) struct Sight<'a> {
    projection: &'a Projection,
    local: Rect,
    total: Homography,
    margin: Rect,
    factor: f32,
    stretch: f32,
}

impl<'a> Sight<'a> {
    pub(crate) fn new(
        projection: &'a Projection,
        local: Rect,
        format: Format,
        factor: f32,
    ) -> Option<Sight<'a>> {
        let frame = Rect::new(0.0, 0.0, format.width as f32, format.height as f32);
        let outline = local.corners();
        let visible = projection.clipped(&outline, frame);
        let stretch = visible
            .iter()
            .filter_map(|corner| projection.stretch(*corner))
            .fold(0.0f32, f32::max)
            * factor;
        if visible.len() < 3 || !(stretch > 0.0 && stretch.is_finite()) {
            return None;
        }
        let total = projection
            .homography()
            .then(Homography::about(Point::ZERO, f64::from(factor)));
        Homography::new(total.matrix().map(|row| row.map(f64::from))).inverse()?;
        Some(Sight {
            projection,
            local,
            total,
            margin: frame.inflate(SLACK / factor),
            factor,
            stretch,
        })
    }

    pub(crate) fn local(&self) -> Rect {
        self.local
    }

    pub(crate) fn stretch(&self) -> f32 {
        self.stretch
    }

    pub(crate) fn matrix(&self, to_local: Affine) -> [[f32; 3]; 3] {
        let rows = Homography::from_affine(to_local).then(self.total).rows();
        let weight: f64 = rows[2].iter().map(|value| value.abs()).sum();
        let rows = match weight > 0.0 {
            true => rows.map(|row| row.map(|value| value / weight)),
            false => rows,
        };
        Homography::new(rows).matrix()
    }

    pub(crate) fn cover(&self, visible: Option<Vec<Point>>, extent: Rect) -> Cover {
        let outline = extent.corners();
        let polygon = match visible {
            Some(polygon) => polygon,
            None if self.projection.crosses_near_plane(&outline) => outline.to_vec(),
            None => return Cover::Whole,
        };
        match self.traced(&polygon) {
            Some(points) => Cover::Within(Mask::path(Path::polygon(points), FillRule::NonZero)),
            None => Cover::Nowhere,
        }
    }

    pub(crate) fn traced(&self, polygon: &[Point]) -> Option<Vec<Point>> {
        let points: Vec<Point> = self
            .projection
            .clipped(polygon, self.margin)
            .into_iter()
            .filter_map(|local| self.projection.map(local))
            .map(|point| point * self.factor)
            .collect();
        (points.len() >= 3).then_some(points)
    }
}
