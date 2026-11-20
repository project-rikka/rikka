use rikka_physics_math::{Aabb, Transform, Vec3};

use super::data::HeightField;
use crate::triangulated::sweep::{overlap_triangles, ray_cast_triangles, shape_cast_triangles};
use crate::{CastHit, MassData, RayInput, ShapeCastInput, ShapeGeometry, ShapeProxy};

impl HeightField {
    pub fn query_triangles(
        &self,
        bounds: Aabb,
        visit: impl FnMut(Vec3, Vec3, Vec3, u32) -> bool,
    ) -> bool {
        self.view().query_triangles(bounds, visit)
    }
}

impl ShapeGeometry for HeightField {
    fn aabb(&self, transform: Transform) -> Aabb {
        self.bounds().transformed(transform)
    }

    fn mass(&self, _: f32) -> MassData {
        MassData::ZERO
    }

    fn ray_cast(&self, ray: &RayInput) -> Option<CastHit> {
        ray_cast_triangles(&self.view(), ray)
    }

    fn shape_cast(&self, cast: &ShapeCastInput<'_>) -> Option<CastHit> {
        shape_cast_triangles(&self.view(), cast)
    }

    fn overlaps(&self, transform: Transform, proxy: &ShapeProxy<'_>) -> bool {
        overlap_triangles(&self.view(), transform, proxy)
    }
}
