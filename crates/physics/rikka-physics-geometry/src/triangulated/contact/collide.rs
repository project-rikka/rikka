use rikka_physics_math::{Aabb, Mat3, Transform};

use super::cache::{CachedFeature, FeatureKey, KeyCollector, TriangulatedContact, WHOLE};
use super::hull_view::{HullStorage, HullView};
use super::pipeline::{ConvexKind, Settings, Target, Work};
use super::scratch::MeshContactScratch;
use crate::triangulated::sweep::TriangleSource;
use crate::{Compound, Manifold, PairCache, Shape, ShapeGeometry, collide};

pub fn collide_triangulated(
    a: &Shape,
    b: &Shape,
    transform: Transform,
    speculative: f32,
    cache: &mut TriangulatedContact,
    scratch: &mut MeshContactScratch,
    out: &mut impl Extend<Manifold>,
) {
    let settings = scratch.settings(speculative);
    let margin = scratch.margin(speculative);
    let MeshContactScratch {
        collector,
        work,
        hull,
        manifolds,
        ..
    } = scratch;
    manifolds.clear();
    let mut context = Context {
        settings,
        margin,
        collector,
        work,
        hull,
        manifolds,
    };
    let triangulated = |shape: &Shape| {
        matches!(
            shape,
            Shape::Mesh(_) | Shape::HeightField(_) | Shape::Compound(_)
        )
    };
    match (a, b) {
        (Shape::Compound(first), Shape::Compound(second)) => {
            context.compounds(first, second, transform, cache);
        }
        (convex, other) if triangulated(other) && convex.as_convex().is_some() => {
            context.convex_against(convex, other, transform, cache);
        }
        (other, convex) if triangulated(other) && convex.as_convex().is_some() => {
            context.convex_against(convex, other, transform.inverse(), cache);
            place(context.manifolds, transform, true);
        }
        _ => {}
    }
    out.extend(context.manifolds.drain(..));
}

struct Context<'s> {
    settings: Settings,
    margin: f32,
    collector: &'s mut KeyCollector,
    work: &'s mut Work,
    hull: &'s mut HullStorage,
    manifolds: &'s mut Vec<Manifold>,
}

fn convex_kind<'a>(shape: &'a Shape, storage: &'a mut HullStorage) -> Option<ConvexKind<'a>> {
    Some(match shape {
        Shape::Sphere(sphere) => ConvexKind::Sphere {
            center: sphere.center,
            radius: sphere.radius,
        },
        Shape::Capsule(capsule) => ConvexKind::Capsule(*capsule),
        Shape::Cuboid(cuboid) => ConvexKind::Hull(HullView::of_cuboid(cuboid, storage)),
        Shape::Cylinder(cylinder) => ConvexKind::Hull(HullView::of_cylinder(cylinder, storage)),
        Shape::Hull(hull) => ConvexKind::Hull(HullView::of_hull(hull)),
        _ => return None,
    })
}

#[inline]
fn is_convex(shape: &Shape) -> bool {
    shape.as_convex().is_some()
}

fn place(manifolds: &mut [Manifold], transform: Transform, flip: bool) {
    if manifolds.is_empty() {
        return;
    }
    let rotation = Mat3::from_quat(transform.q);
    for manifold in manifolds {
        let normal = rotation * manifold.normal;
        manifold.normal = if flip { -normal } else { normal };
        for point in &mut manifold.points[..manifold.count as usize] {
            point.point = rotation * point.point + transform.p;
        }
    }
}

fn collect_triangles(
    source: &impl TriangleSource,
    bounds: Aabb,
    a: u32,
    b: u32,
    keys: &mut KeyCollector,
) -> bool {
    source.overlapping(bounds, |triangle| {
        keys.push(FeatureKey::new(a, b, triangle))
    })
}

fn collect_child(shape: &Shape, bounds: Aabb, a: u32, b: u32, keys: &mut KeyCollector) -> bool {
    match shape {
        Shape::Mesh(mesh) => collect_triangles(&mesh.view(), bounds, a, b, keys),
        shape if is_convex(shape) => keys.push(FeatureKey::new(a, b, WHOLE)),
        _ => true,
    }
}

impl Context<'_> {
    fn triangles(
        &mut self,
        target: &Target<'_, impl TriangleSource>,
        convex: &Shape,
        (entries, pairs): (&mut [CachedFeature], &mut [PairCache]),
    ) {
        let Some(kind) = convex_kind(convex, self.hull) else {
            return;
        };
        self.work
            .collide(target, &kind, self.settings, entries, pairs, self.manifolds);
    }

    fn pair(
        &mut self,
        a: &Shape,
        b: &Shape,
        transform: Transform,
        (entries, pairs): (&mut [CachedFeature], &mut [PairCache]),
        material: u32,
    ) {
        let mut manifold = collide(a, b, transform, self.settings.speculative, &mut pairs[0]);
        if manifold.is_empty() {
            return;
        }
        let salt = entries[0].key.salt();
        for point in &mut manifold.points[..manifold.count as usize] {
            point.feature ^= salt;
        }
        manifold.material = material;
        self.manifolds.push(manifold);
    }

    fn convex_against(
        &mut self,
        convex: &Shape,
        other: &Shape,
        transform: Transform,
        cache: &mut TriangulatedContact,
    ) {
        let bounds = convex.aabb(transform.inverse());
        let margin = self.margin;
        let paired = matches!(convex, Shape::Capsule(_));
        let collector = &mut *self.collector;
        match other {
            Shape::Mesh(mesh) => {
                let view = mesh.view();
                cache.refresh(bounds, margin, paired, collector, |query, keys| {
                    collect_triangles(&view, query, 0, 0, keys);
                });
                self.triangles(&Target::whole(&view, transform), convex, cache.features());
            }
            Shape::HeightField(field) => {
                let view = field.view();
                cache.refresh(bounds, margin, paired, collector, |query, keys| {
                    collect_triangles(&view, query, 0, 0, keys);
                });
                self.triangles(&Target::whole(&view, transform), convex, cache.features());
            }
            Shape::Compound(compound) => {
                cache.refresh(bounds, margin, true, collector, |query, keys| {
                    compound.query_children(query, |index| {
                        let child = compound.child(index);
                        let local = query.transformed(child.transform.inverse());
                        collect_child(child.shape, local, 0, index, keys)
                    });
                });
                for group in cache.groups() {
                    let key = group.0[0].key;
                    let child = compound.child(key.b);
                    let child_transform = transform * child.transform;
                    match child.shape {
                        Shape::Mesh(mesh) => {
                            let target = Target {
                                source: &mesh.view(),
                                salt: key.salt(),
                                material_base: child.material,
                                transform: child_transform,
                            };
                            self.triangles(&target, convex, group);
                        }
                        shape => self.pair(convex, shape, child_transform, group, child.material),
                    }
                }
            }
            _ => {}
        }
    }

    fn compounds(
        &mut self,
        first: &Compound,
        second: &Compound,
        transform: Transform,
        cache: &mut TriangulatedContact,
    ) {
        let bounds = second.aabb(transform);
        let margin = self.margin;
        let inverse = transform.inverse();
        cache.refresh(bounds, margin, true, self.collector, |query, keys| {
            first.query_children(query, |i| {
                let child_a = first.child(i);
                let a_in_b = inverse * child_a.transform;
                let bounds_a = child_a.shape.aabb(a_in_b).inflated(margin);
                second.query_children(bounds_a, |j| {
                    let child_b = second.child(j);
                    match (child_a.shape, child_b.shape) {
                        (a, Shape::Mesh(mesh)) if is_convex(a) => {
                            let local = a
                                .aabb(child_b.transform.inverse() * a_in_b)
                                .inflated(margin);
                            collect_triangles(&mesh.view(), local, i, j, keys)
                        }
                        (Shape::Mesh(mesh), b) if is_convex(b) => {
                            let local = b
                                .aabb(child_a.transform.inverse() * transform * child_b.transform)
                                .inflated(margin);
                            collect_triangles(&mesh.view(), local, i, j, keys)
                        }
                        (a, b) if is_convex(a) && is_convex(b) => {
                            keys.push(FeatureKey::new(i, j, WHOLE))
                        }
                        _ => true,
                    }
                })
            });
        });
        for group in cache.groups() {
            let key = group.0[0].key;
            let child_a = first.child(key.a);
            let child_b = second.child(key.b);
            let b_in_a = child_a.transform.inverse() * transform * child_b.transform;
            let start = self.manifolds.len();
            match (child_a.shape, child_b.shape) {
                (a, Shape::Mesh(mesh)) => {
                    let target = Target {
                        source: &mesh.view(),
                        salt: key.salt(),
                        material_base: child_b.material,
                        transform: b_in_a,
                    };
                    self.triangles(&target, a, group);
                    place(&mut self.manifolds[start..], child_a.transform, false);
                }
                (Shape::Mesh(mesh), b) => {
                    let target = Target {
                        source: &mesh.view(),
                        salt: key.salt(),
                        material_base: child_a.material,
                        transform: b_in_a.inverse(),
                    };
                    self.triangles(&target, b, group);
                    let back = transform * child_b.transform;
                    place(&mut self.manifolds[start..], back, true);
                }
                (a, b) => {
                    self.pair(a, b, b_in_a, group, child_b.material);
                    place(&mut self.manifolds[start..], child_a.transform, false);
                }
            }
        }
    }
}
