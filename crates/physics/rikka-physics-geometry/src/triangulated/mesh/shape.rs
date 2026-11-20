use rikka_physics_math::{Aabb, Transform, Vec3};

use super::data::TriangleMesh;
use super::view::{MeshView, scale_bounds};
use crate::triangulated::sweep::{overlap_triangles, ray_cast_triangles, shape_cast_triangles};
use crate::triangulated::triangle::Triangle;
use crate::triangulated::volume::closed_surface_mass;
use crate::{
    CastHit, Error, MassData, RayInput, Result, ShapeCastInput, ShapeGeometry, ShapeProxy,
};

const MIN_SCALE: f32 = 1.0e-6;

#[derive(Clone, Debug, PartialEq)]
pub struct MeshShape {
    pub mesh: TriangleMesh,
    pub scale: Vec3,
}

impl MeshShape {
    pub fn new(mesh: TriangleMesh) -> Self {
        Self {
            mesh,
            scale: Vec3::ONE,
        }
    }

    pub fn scaled(mesh: TriangleMesh, scale: Vec3) -> Self {
        Self { mesh, scale }
    }

    pub fn validate(&self) -> Result<()> {
        for value in self.scale.to_array() {
            if !value.is_finite() || value.abs() < MIN_SCALE {
                return Err(Error::InvalidDimension {
                    what: "mesh scale",
                    value,
                });
            }
        }
        Ok(())
    }

    #[inline]
    pub fn bounds(&self) -> Aabb {
        scale_bounds(&self.mesh.bounds(), self.scale)
    }

    #[inline]
    pub(crate) fn view(&self) -> MeshView<'_> {
        let (slices, tree) = self.mesh.parts();
        MeshView::new(slices, tree, self.scale)
    }

    pub fn triangle(&self, index: u32) -> Triangle {
        self.view().triangle(index)
    }

    pub fn query_triangles(
        &self,
        bounds: Aabb,
        visit: impl FnMut(Vec3, Vec3, Vec3, u32) -> bool,
    ) -> bool {
        self.view().query_triangles(bounds, visit)
    }
}

impl ShapeGeometry for MeshShape {
    fn aabb(&self, transform: Transform) -> Aabb {
        self.bounds().transformed(transform)
    }

    fn mass(&self, density: f32) -> MassData {
        if !self.mesh.is_closed() {
            return MassData::ZERO;
        }
        let reference = self.bounds().center();
        let view = self.view();
        closed_surface_mass(
            reference,
            density,
            (0..self.mesh.triangle_count()).map(|index| view.triangle(index).vertices),
        )
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
