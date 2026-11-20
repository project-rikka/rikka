use rikka_model::glam::{Mat4, Vec3, Vec4};
use rikka_types::{Point, Rect};

const NEAR: f32 = 1.0;
const FURTHER: f32 = 0.5;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Lens {
    view_projection: Mat4,
    inverse: Mat4,
    frame: Rect,
}

impl Lens {
    pub fn new(view_projection: Mat4, frame: Rect) -> Self {
        Self {
            view_projection,
            inverse: view_projection.inverse(),
            frame,
        }
    }

    pub fn frame(&self) -> Rect {
        self.frame
    }

    pub fn project(&self, point: Vec3) -> Option<Point> {
        project(self.view_projection, self.frame, point)
    }

    pub fn depth(&self, point: Vec3) -> f32 {
        let clip = self.view_projection * point.extend(1.0);
        match clip.w.abs() > 1e-9 {
            true => clip.z / clip.w,
            false => NEAR,
        }
    }

    pub(super) fn unproject(&self, at: Point, depth: f32) -> Vec3 {
        let x = (at.x - self.frame.x()) / self.frame.width().max(1.0) * 2.0 - 1.0;
        let y = (at.y - self.frame.y()) / self.frame.height().max(1.0) * 2.0 - 1.0;
        let unprojected = self.inverse * Vec4::new(x, y, depth, 1.0);
        unprojected.truncate() / unprojected.w
    }

    pub fn ray(&self, at: Point) -> (Vec3, Vec3) {
        let near = self.unproject(at, NEAR);
        let further = self.unproject(at, FURTHER);
        (near, (further - near).normalize_or_zero())
    }

    pub fn sideways(&self, point: Vec3) -> (Vec3, Vec3, f32) {
        let depth = self.depth(point);
        let Some(at) = self.project(point) else {
            return (Vec3::X, Vec3::Y, 0.001);
        };
        let here = self.unproject(at, depth);
        let right = self.unproject(at + Point::new(1.0, 0.0), depth) - here;
        let up = here - self.unproject(at + Point::new(0.0, 1.0), depth);
        (
            right.normalize_or(Vec3::X),
            up.normalize_or(Vec3::Y),
            right.length().max(1e-6),
        )
    }

    pub fn ahead(&self, point: Vec3) -> Vec3 {
        let at = self.project(point).unwrap_or(self.frame.center());
        self.ray(at).1
    }
}

pub fn project(view_projection: Mat4, frame: Rect, point: Vec3) -> Option<Point> {
    let clip = view_projection * point.extend(1.0);
    if clip.w <= 1e-6 {
        return None;
    }
    let ndc = clip.truncate() / clip.w;
    if ndc.x.abs() > 1.5 || ndc.y.abs() > 1.5 {
        return None;
    }
    Some(Point::new(
        frame.x() + (ndc.x * 0.5 + 0.5) * frame.width(),
        frame.y() + (ndc.y * 0.5 + 0.5) * frame.height(),
    ))
}
