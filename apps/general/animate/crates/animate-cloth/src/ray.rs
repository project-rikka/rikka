use rikka_model_core::glam::Vec3;

const GRAZING: f32 = 1e-6;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ray {
    pub origin: Vec3,
    pub direction: Vec3,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Crossing {
    pub along: f32,
    pub u: f32,
    pub v: f32,
}

impl Ray {
    #[must_use]
    pub fn crossing(&self, [a, b, c]: [Vec3; 3]) -> Option<Crossing> {
        let (ab, ac) = (b - a, c - a);
        let across = self.direction.cross(ac);
        let determinant = ab.dot(across);
        if determinant.abs() <= GRAZING * ab.cross(ac).length() {
            return None;
        }
        let inverse = determinant.recip();
        let from = self.origin - a;
        let u = from.dot(across) * inverse;
        if !(0.0..=1.0).contains(&u) {
            return None;
        }
        let up = from.cross(ab);
        let v = self.direction.dot(up) * inverse;
        if v < 0.0 || u + v > 1.0 {
            return None;
        }
        let along = ac.dot(up) * inverse;
        (along > 0.0).then_some(Crossing { along, u, v })
    }

    #[must_use]
    pub fn at(&self, along: f32) -> Vec3 {
        self.origin + self.direction * along
    }
}
