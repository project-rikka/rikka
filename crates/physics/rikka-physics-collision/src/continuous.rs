use rikka_physics_geometry::{
    MeshShape, ProxyBuffer, Shape, ShapeGeometry, ShapeProxy, Sweep, ToiInput, ToiOutput, ToiState,
    Tolerance, time_of_impact,
};
use rikka_physics_math::{Aabb, Transform, Vec3};

#[derive(Clone, Copy, Debug)]
pub struct SweptShape<'a> {
    pub shape: &'a Shape,
    pub sweep: Sweep,
    pub bounds: Aabb,
}

impl<'a> SweptShape<'a> {
    pub fn new(shape: &'a Shape, sweep: Sweep, until: f32) -> Self {
        let bounds = shape
            .aabb(sweep.transform(0.0))
            .union(&shape.aabb(sweep.transform(until)));
        Self {
            shape,
            sweep,
            bounds,
        }
    }
}

struct Fast<'a> {
    proxy: ShapeProxy<'a>,
    sweep: Sweep,
    centroid: Vec3,
    bounds: Aabb,
    fallback: f32,
    tolerance: Tolerance,
}

impl Fast<'_> {
    fn toi(
        &self,
        target: &ShapeProxy<'_>,
        sweep: Sweep,
        proxy: &ShapeProxy<'_>,
        best: f32,
    ) -> ToiOutput {
        time_of_impact(&ToiInput {
            a: target,
            b: proxy,
            sweep_a: sweep,
            sweep_b: self.sweep,
            max_fraction: best,
            tolerance: self.tolerance,
        })
    }

    fn convex(&self, target: &ShapeProxy<'_>, sweep: Sweep, best: f32) -> Option<f32> {
        accepted(&self.toi(target, sweep, &self.proxy, best), best)
    }

    fn mesh(&self, mesh: &MeshShape, transform: Transform, best: &mut f32) -> bool {
        let local = self.bounds.transformed(transform.inverse());
        let c1 = transform.apply_inverse(self.sweep.transform(0.0).apply(self.centroid));
        let c2 = transform.apply_inverse(self.sweep.transform(1.0).apply(self.centroid));
        let sweep = Sweep::fixed(transform, Vec3::ZERO);
        let mut hit = false;
        mesh.query_triangles(local, |a, b, c, _| {
            if let Some(fraction) = self.triangle([a, b, c], c1, c2, sweep, *best) {
                *best = fraction;
                hit = true;
            }
            true
        });
        hit
    }

    fn triangle(
        &self,
        vertices: [Vec3; 3],
        c1: Vec3,
        c2: Vec3,
        sweep: Sweep,
        best: f32,
    ) -> Option<f32> {
        let [a, b, c] = vertices;
        let normal = (b - a).cross(c - a).normalize_or_zero();
        let offset1 = normal.dot(c1 - a);
        let offset2 = normal.dot(c2 - a);
        if offset1 < 0.0 || (offset1 - offset2 < self.fallback && offset2 > self.fallback) {
            return None;
        }
        let triangle = ShapeProxy::new(&vertices, 0.0);
        let output = self.toi(&triangle, sweep, &self.proxy, best);
        if output.fraction > 0.0 {
            return accepted(&output, best);
        }
        let centroid = [self.centroid];
        let fallback = ShapeProxy::new(&centroid, self.fallback + self.tolerance.linear_slop());
        accepted(&self.toi(&triangle, sweep, &fallback, best), best)
    }

    fn plane(&self, normal: Vec3, offset: f32, best: f32) -> Option<f32> {
        let low = |time: f32| {
            let bounds = self.proxy_bounds(self.sweep.transform(time));
            normal.dot(bounds.center()) - bounds.extent().dot(normal.abs()) - offset
        };
        let (s0, s1) = (low(0.0), low(1.0));
        let fraction = s0 / (s0 - s1);
        (s0 > 0.0 && s1 < 0.0 && fraction < best).then_some(fraction)
    }

    fn proxy_bounds(&self, transform: Transform) -> Aabb {
        Aabb::from_points(
            self.proxy
                .points
                .iter()
                .map(|&point| transform.apply(point)),
        )
        .inflated(self.proxy.radius)
    }
}

fn accepted(output: &ToiOutput, best: f32) -> Option<f32> {
    let valid = matches!(output.state, ToiState::Hit | ToiState::Failed);
    (valid && output.fraction > 0.0 && output.fraction < best).then_some(output.fraction)
}

pub fn shape_time_of_impact(
    target: &Shape,
    target_transform: Transform,
    fast: &SweptShape<'_>,
    max_fraction: f32,
    tolerance: Tolerance,
) -> Option<f32> {
    let mut fast_buffer = ProxyBuffer::default();
    let proxy = fast.shape.proxy(&mut fast_buffer)?;
    let local = fast.shape.aabb(Transform::IDENTITY);
    let half = local.extent();
    let context = Fast {
        proxy,
        sweep: fast.sweep,
        centroid: local.center(),
        bounds: fast.bounds,
        fallback: (0.5 * half.min_element()).max(tolerance.linear_slop()),
        tolerance,
    };
    let mut target_buffer = ProxyBuffer::default();
    let mut best = max_fraction;
    let hit = match target {
        Shape::Mesh(mesh) => context.mesh(mesh, target_transform, &mut best),
        Shape::HeightField(field) => {
            let local = context.bounds.transformed(target_transform.inverse());
            let c1 =
                target_transform.apply_inverse(fast.sweep.transform(0.0).apply(context.centroid));
            let c2 =
                target_transform.apply_inverse(fast.sweep.transform(1.0).apply(context.centroid));
            let sweep = Sweep::fixed(target_transform, Vec3::ZERO);
            let mut hit = false;
            field.query_triangles(local, |a, b, c, _| {
                if let Some(fraction) = context.triangle([a, b, c], c1, c2, sweep, best) {
                    best = fraction;
                    hit = true;
                }
                true
            });
            hit
        }
        Shape::Compound(compound) => {
            let local = context.bounds.transformed(target_transform.inverse());
            let mut hit = false;
            compound.query_children(local, |index| {
                let child = compound.child(index);
                let transform = target_transform.mul(&child.transform);
                if let Shape::Mesh(mesh) = child.shape {
                    hit |= context.mesh(mesh, transform, &mut best);
                } else if let Some(other) = child.shape.proxy(&mut target_buffer)
                    && let Some(fraction) =
                        context.convex(&other, Sweep::fixed(transform, Vec3::ZERO), best)
                {
                    best = fraction;
                    hit = true;
                }
                true
            });
            hit
        }
        Shape::Plane(plane) => {
            let normal = target_transform.rotate(plane.normal);
            let offset = plane.offset + normal.dot(target_transform.p);
            context
                .plane(normal, offset, best)
                .map(|fraction| best = fraction)
                .is_some()
        }
        _ => {
            let other = target.proxy(&mut target_buffer)?;
            context
                .convex(&other, Sweep::fixed(target_transform, Vec3::ZERO), best)
                .map(|fraction| best = fraction)
                .is_some()
        }
    };
    hit.then_some(best)
}
