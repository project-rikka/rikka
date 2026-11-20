use rikka_physics_geometry::Manifold;

use super::recycle::cache_pose;
use crate::contact::{
    ContactGeometry, ContactHeader, STARTED_TOUCHING, STOPPED_TOUCHING, TOUCHING,
};
use crate::{BodyFrame, Collider, ContactManifold, MixedMaterial};

#[inline]
fn material(
    header: &ContactHeader,
    world: &Manifold,
    a: &BodyFrame,
    b: &BodyFrame,
    colliders: &[Collider],
) -> MixedMaterial {
    MixedMaterial::mix(
        colliders[header.collider_a as usize].material(world.material),
        a.transform.q,
        colliders[header.collider_b as usize].material(world.material),
        b.transform.q,
    )
}

#[inline]
fn update_touching(header: &mut ContactHeader, touching: bool) -> bool {
    let was = header.flags & TOUCHING != 0;
    if touching == was {
        return false;
    }
    if touching {
        header.flags |= TOUCHING | STARTED_TOUCHING;
    } else {
        header.flags &= !TOUCHING;
        header.flags |= STOPPED_TOUCHING;
    }
    true
}

#[inline]
pub(crate) fn finish(
    header: &mut ContactHeader,
    contact: &mut ContactGeometry,
    world: &Manifold,
    a: &BodyFrame,
    b: &BodyFrame,
    colliders: &[Collider],
) -> bool {
    cache_pose(header, &a.transform, &b.transform);
    let mixed = material(header, world, a, b, colliders);
    contact.manifold.replace(world, a.center, b.center, mixed);
    header.points = contact.manifold.count as u8;
    update_touching(header, world.count > 0)
}

pub(crate) fn finish_many(
    header: &mut ContactHeader,
    contact: &mut ContactGeometry,
    manifolds: &[Manifold],
    previous: &[ContactManifold],
    (a, b): (&BodyFrame, &BodyFrame),
    colliders: &[Collider],
    extras: &mut Vec<ContactManifold>,
) -> bool {
    cache_pose(header, &a.transform, &b.transform);
    let mut touching = false;
    let empty = Manifold::default();
    let convert = |world: &Manifold, header: &ContactHeader| {
        let mixed = material(header, world, a, b, colliders);
        ContactManifold::from_world(world, a.center, b.center, mixed, previous)
    };
    let primary = manifolds.first().unwrap_or(&empty);
    contact.manifold = convert(primary, header);
    header.points = contact.manifold.count as u8;
    touching |= primary.count > 0;
    for world in manifolds.iter().skip(1) {
        extras.push(convert(world, header));
        touching |= world.count > 0;
    }
    update_touching(header, touching)
}

#[inline]
pub(crate) fn trim_unreachable(world: &mut Manifold, a: &BodyFrame, b: &BodyFrame, base: f32) {
    let normal = world.normal;
    let mut write = 0;
    for index in 0..world.count as usize {
        let point = world.points[index];
        let travel_a = a.motion + a.turn.cross(point.point - a.center);
        let travel_b = b.motion + b.turn.cross(point.point - b.center);
        let closing = (travel_a - travel_b).dot(normal);
        if point.separation <= base.max(closing) {
            world.points[write] = point;
            write += 1;
        }
    }
    world.count = write as u32;
}
