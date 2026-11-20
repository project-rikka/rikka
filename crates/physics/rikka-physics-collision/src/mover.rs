use rikka_physics_geometry::{
    DistanceInput, ProxyBuffer, Shape, ShapeProxy, SimplexCache, Tolerance, shape_distance,
};
use rikka_physics_math::{Aabb, Plane, Transform, Vec3, perpendicular};

const SOLVER_ITERATIONS: u32 = 20;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mover {
    pub center1: Vec3,
    pub center2: Vec3,
    pub radius: f32,
}

impl Mover {
    pub fn new(center1: Vec3, center2: Vec3, radius: f32) -> Self {
        Self {
            center1,
            center2,
            radius,
        }
    }

    pub fn translated(&self, offset: Vec3) -> Self {
        Self::new(self.center1 + offset, self.center2 + offset, self.radius)
    }

    pub fn bounds(&self) -> Aabb {
        Aabb::new(
            self.center1.min(self.center2),
            self.center1.max(self.center2),
        )
        .inflated(self.radius)
    }

    fn localized(&self, transform: Transform) -> Self {
        Self::new(
            transform.apply_inverse(self.center1),
            transform.apply_inverse(self.center2),
            self.radius,
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MoverPlane {
    pub plane: Plane,
    pub point: Vec3,
    pub triangle: u32,
    pub child: u32,
    pub material: u32,
}

impl MoverPlane {
    fn transformed(mut self, transform: Transform) -> Self {
        self.plane.normal = transform.rotate(self.plane.normal);
        self.point = transform.apply(self.point);
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CollisionPlane {
    pub plane: Plane,
    pub push_limit: f32,
    pub push: f32,
    pub clip_velocity: bool,
}

impl CollisionPlane {
    pub fn rigid(plane: Plane) -> Self {
        Self {
            plane,
            push_limit: f32::MAX,
            push: 0.0,
            clip_velocity: true,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlaneSolution {
    pub delta: Vec3,
    pub iterations: u32,
}

pub fn solve_planes(
    target: Vec3,
    planes: &mut [CollisionPlane],
    tolerance: Tolerance,
) -> PlaneSolution {
    let slop = tolerance.linear_slop();
    for plane in planes.iter_mut() {
        plane.push = 0.0;
    }
    let mut delta = target;
    let mut iterations = 0;
    while iterations < SOLVER_ITERATIONS {
        let mut total = 0.0;
        for plane in planes.iter_mut() {
            let separation = plane.plane.separation(delta) + slop;
            let previous = plane.push;
            plane.push = (plane.push - separation).clamp(0.0, plane.push_limit);
            let push = plane.push - previous;
            delta += push * plane.plane.normal;
            total += push.abs();
        }
        if total < slop {
            break;
        }
        iterations += 1;
    }
    PlaneSolution { delta, iterations }
}

pub fn clip_vector(vector: Vec3, planes: &[CollisionPlane]) -> Vec3 {
    planes
        .iter()
        .filter(|plane| plane.push != 0.0 && plane.clip_velocity)
        .fold(vector, |v, plane| {
            v - v.dot(plane.plane.normal).min(0.0) * plane.plane.normal
        })
}

pub fn collide_mover(
    shape: &Shape,
    transform: Transform,
    mover: &Mover,
    tolerance: Tolerance,
    mut visit: impl FnMut(MoverPlane) -> bool,
) -> bool {
    let local = mover.localized(transform);
    collide_local(shape, &local, tolerance, 0, &mut |plane| {
        visit(plane.transformed(transform))
    })
}

fn collide_local(
    shape: &Shape,
    mover: &Mover,
    tolerance: Tolerance,
    child: u32,
    visit: &mut dyn FnMut(MoverPlane) -> bool,
) -> bool {
    match shape {
        Shape::Mesh(mesh) => mesh.query_triangles(mover.bounds(), |a, b, c, triangle| {
            let material = mesh.triangle(triangle).material as u32;
            triangle_plane([a, b, c], mover).is_none_or(|(plane, point)| {
                visit(MoverPlane {
                    plane,
                    point,
                    triangle,
                    child,
                    material,
                })
            })
        }),
        Shape::HeightField(field) => field.query_triangles(mover.bounds(), |a, b, c, triangle| {
            let material = field.triangle(triangle).material as u32;
            triangle_plane([a, b, c], mover).is_none_or(|(plane, point)| {
                visit(MoverPlane {
                    plane,
                    point,
                    triangle,
                    child,
                    material,
                })
            })
        }),
        Shape::Compound(compound) => compound.query_children(mover.bounds(), |index| {
            let entry = compound.child(index);
            let local = mover.localized(entry.transform);
            collide_local(entry.shape, &local, tolerance, index, &mut |mut plane| {
                plane = plane.transformed(entry.transform);
                if !matches!(entry.shape, Shape::Mesh(_)) {
                    plane.material = entry.material;
                } else {
                    plane.material += entry.material;
                }
                visit(plane)
            })
        }),
        Shape::Plane(surface) => {
            let separation = surface
                .separation(mover.center1)
                .min(surface.separation(mover.center2));
            if separation > mover.radius {
                return true;
            }
            let deepest = if surface.separation(mover.center1) <= surface.separation(mover.center2)
            {
                mover.center1
            } else {
                mover.center2
            };
            visit(MoverPlane {
                plane: Plane::new(surface.normal, mover.radius - separation),
                point: surface.project(deepest),
                triangle: 0,
                child,
                material: 0,
            })
        }
        _ => {
            let mut buffer = ProxyBuffer::default();
            let Some(proxy) = shape.proxy(&mut buffer) else {
                return true;
            };
            convex_plane(&proxy, mover, tolerance).is_none_or(|(plane, point)| {
                visit(MoverPlane {
                    plane,
                    point,
                    triangle: 0,
                    child,
                    material: 0,
                })
            })
        }
    }
}

fn segment_distance(
    proxy: &ShapeProxy<'_>,
    mover: &Mover,
) -> rikka_physics_geometry::DistanceOutput {
    let segment = [mover.center1, mover.center2];
    let segment = ShapeProxy::new(&segment, 0.0);
    let core = ShapeProxy::new(proxy.points, 0.0);
    let mut cache = SimplexCache::EMPTY;
    shape_distance(
        &DistanceInput::new(&core, &segment, Transform::IDENTITY),
        &mut cache,
    )
}

fn convex_plane(
    proxy: &ShapeProxy<'_>,
    mover: &Mover,
    tolerance: Tolerance,
) -> Option<(Plane, Vec3)> {
    let total = mover.radius + proxy.radius;
    let output = segment_distance(proxy, mover);
    if output.distance > total {
        return None;
    }
    if output.distance < tolerance.linear_slop() {
        if proxy.radius == 0.0 {
            return None;
        }
        let axis = mover.center2 - mover.center1;
        let normal = if axis.length() > tolerance.linear_slop() {
            perpendicular(axis.normalize())
        } else {
            Vec3::Y
        };
        return Some((Plane::new(normal, total), output.point_a));
    }
    Some((
        Plane::new(output.normal, total - output.distance),
        output.point_a + proxy.radius * output.normal,
    ))
}

fn triangle_plane(vertices: [Vec3; 3], mover: &Mover) -> Option<(Plane, Vec3)> {
    let [a, b, c] = vertices;
    let center = 0.5 * (mover.center1 + mover.center2);
    if (b - a).cross(c - a).dot(center - a) < 0.0 {
        return None;
    }
    let output = segment_distance(&ShapeProxy::new(&vertices, 0.0), mover);
    if output.distance == 0.0 || output.distance > mover.radius {
        return None;
    }
    Some((
        Plane::new(output.normal, mover.radius - output.distance),
        output.point_a,
    ))
}
