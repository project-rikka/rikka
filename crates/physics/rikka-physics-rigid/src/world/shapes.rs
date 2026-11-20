use rikka_physics_collision::{Collider, Filter, Material, ProxyKind, ShapeKey};
use rikka_physics_geometry::{MassData, ProxyBuffer, Shape, ShapeGeometry};
use rikka_physics_math::{Aabb, Transform, Vec3};

use super::RigidWorld;
use crate::body::{BodyKind, MassSpec, NONE};
use crate::shape_record::ShapeRecord;
use crate::solver::sym::Sym3;
use crate::{BodyId, Error, Result, ShapeDef, ShapeId};

fn proxy_kind(kind: BodyKind) -> ProxyKind {
    match kind {
        BodyKind::Static => ProxyKind::Static,
        BodyKind::Kinematic => ProxyKind::Kinematic,
        BodyKind::Dynamic => ProxyKind::Dynamic,
    }
}

impl RigidWorld {
    pub(crate) fn shape_index(&self, id: ShapeId) -> Result<u32> {
        if self.shapes.is_alive(id.index, id.generation) {
            Ok(id.index)
        } else {
            Err(Error::UnknownShape(id))
        }
    }

    pub(crate) fn shape_id(&self, index: u32) -> ShapeId {
        ShapeId {
            index,
            generation: self.shapes.generation(index),
        }
    }

    pub(crate) fn fat_bounds(&self, shape: u32, transform: Transform) -> (Aabb, Aabb) {
        let collider = &self.colliders[shape as usize];
        let frame = &self.frames[collider.owner as usize];
        let shape_bounds = collider.shape.aabb(transform);
        let margin = self
            .config
            .tolerance
            .aabb_margin(shape_bounds.size().max_element());
        let bounds = frame.swept(shape_bounds);
        (bounds, bounds.inflated(margin))
    }

    pub(crate) fn attach_shape(&mut self, body: u32, def: ShapeDef) -> u32 {
        let record = *self.bodies.get(body);
        let (index, _) = self.shapes.insert(ShapeRecord {
            body,
            prev: NONE,
            next: record.shape_head,
            density: def.density,
        });
        if record.shape_head != NONE {
            self.shapes.get_mut(record.shape_head).prev = index;
        }
        let body_record = self.bodies.get_mut(body);
        body_record.shape_head = index;
        body_record.shape_count += 1;
        let collider = Collider {
            shape: def.shape,
            materials: def.materials,
            filter: def.filter,
            sensor: def.sensor,
            owner: body,
            contact_events: def.contact_events,
            hit_events: def.hit_events,
            sensor_events: def.sensor_events,
            contact_response: def.contact_response,
        };
        if index as usize == self.colliders.len() {
            self.colliders.push(collider);
        } else {
            self.colliders[index as usize] = collider;
        }
        let transform = self.pose_of(body).transform;
        let (_, fat) = self.fat_bounds(index, transform);
        let collider = &self.colliders[index as usize];
        self.broadphase.create(
            index,
            proxy_kind(record.kind),
            fat,
            collider.filter,
            body,
            collider.responds(),
        );
        if collider.sensor {
            self.sensors.add(ShapeKey {
                collider: index,
                generation: self.shapes.generation(index),
            });
        }
        index
    }

    pub(crate) fn detach_shape(&mut self, shape: u32) {
        let mut key = self.bodies.get(self.shapes.get(shape).body).contact_head;
        while key != NONE {
            let id = key >> 1;
            let contact = *self.contacts.get(id);
            key = contact.edges[(key & 1) as usize].next;
            if contact.shape_a == shape || contact.shape_b == shape {
                self.destroy_contact(id, false);
            }
        }
        let record = *self.shapes.get(shape);
        if record.prev != NONE {
            self.shapes.get_mut(record.prev).next = record.next;
        } else {
            self.bodies.get_mut(record.body).shape_head = record.next;
        }
        if record.next != NONE {
            self.shapes.get_mut(record.next).prev = record.prev;
        }
        self.bodies.get_mut(record.body).shape_count -= 1;
        self.broadphase.destroy(shape);
        self.sensors.remove(shape);
        self.shapes.remove(shape);
    }

    pub fn create_shape(&mut self, body: BodyId, def: impl Into<ShapeDef>) -> Result<ShapeId> {
        let index = self.body_index(body)?;
        let def = def.into();
        if !def.materials.iter().all(Material::is_valid) {
            return Err(Error::InvalidMaterial);
        }
        self.wake_body(index);
        let shape = self.attach_shape(index, def);
        self.update_mass(index);
        Ok(self.shape_id(shape))
    }

    pub fn destroy_shape(&mut self, id: ShapeId) -> Result<()> {
        let shape = self.shape_index(id)?;
        let body = self.shapes.get(shape).body;
        self.wake_body(body);
        self.detach_shape(shape);
        self.update_mass(body);
        Ok(())
    }

    pub fn body_shapes(&self, id: BodyId) -> Result<impl Iterator<Item = ShapeId> + '_> {
        let mut shape = self.bodies.get(self.body_index(id)?).shape_head;
        Ok(std::iter::from_fn(move || {
            (shape != NONE).then(|| {
                let current = shape;
                shape = self.shapes.get(current).next;
                self.shape_id(current)
            })
        }))
    }

    pub fn shape_body(&self, id: ShapeId) -> Result<BodyId> {
        let shape = self.shape_index(id)?;
        Ok(self.body_id(self.shapes.get(shape).body))
    }

    fn collider(&self, id: ShapeId) -> Result<&Collider> {
        Ok(&self.colliders[self.shape_index(id)? as usize])
    }

    pub fn shape_material(&self, id: ShapeId) -> Result<Material> {
        Ok(self.collider(id)?.materials[0])
    }

    pub fn shape_materials(&self, id: ShapeId) -> Result<&[Material]> {
        Ok(&self.collider(id)?.materials)
    }

    pub fn shape_filter(&self, id: ShapeId) -> Result<Filter> {
        Ok(self.collider(id)?.filter)
    }

    pub fn shape_density(&self, id: ShapeId) -> Result<f32> {
        Ok(self.shapes.get(self.shape_index(id)?).density)
    }

    pub fn is_sensor(&self, id: ShapeId) -> Result<bool> {
        Ok(self.collider(id)?.sensor)
    }

    pub fn shape_def(&self, id: ShapeId) -> Result<ShapeDef> {
        let collider = self.collider(id)?;
        Ok(ShapeDef::new(collider.shape.clone())
            .materials(collider.materials.iter().copied())
            .filter(collider.filter)
            .sensor(collider.sensor)
            .density(self.shape_density(id)?)
            .contact_events(collider.contact_events)
            .hit_events(collider.hit_events)
            .sensor_events(collider.sensor_events)
            .contact_response(collider.contact_response))
    }

    pub fn shape(&self, id: ShapeId) -> Result<&Shape> {
        Ok(&self.colliders[self.shape_index(id)? as usize].shape)
    }

    pub fn set_material(&mut self, id: ShapeId, material: Material) -> Result<()> {
        if !material.is_valid() {
            return Err(Error::InvalidMaterial);
        }
        let shape = self.shape_index(id)?;
        self.colliders[shape as usize].materials[0] = material;
        Ok(())
    }

    pub fn set_filter(&mut self, id: ShapeId, filter: Filter) -> Result<()> {
        let shape = self.shape_index(id)?;
        self.colliders[shape as usize].filter = filter;
        self.broadphase.set_filter(shape, filter);
        Ok(())
    }

    pub(crate) fn refresh_proxies(&mut self, body: u32, forced: bool) {
        let transform = self.pose_of(body).transform;
        let mut shape = self.bodies.get(body).shape_head;
        while shape != NONE {
            let (bounds, fat) = self.fat_bounds(shape, transform);
            if forced || !self.broadphase.fat(shape).contains(&bounds) {
                self.broadphase.move_proxy(shape, fat);
            }
            shape = self.shapes.get(shape).next;
        }
    }

    pub(crate) fn update_mass(&mut self, body: u32) {
        let record = *self.bodies.get(body);
        let mut parts = Vec::with_capacity(record.shape_count as usize);
        let mut local_bounds = Aabb::EMPTY;
        let mut shape = record.shape_head;
        while shape != NONE {
            let collider = &self.colliders[shape as usize];
            if !collider.sensor {
                parts.push(collider.shape.mass(self.shapes.get(shape).density));
            }
            local_bounds = local_bounds.union(&collider.shape.aabb(Transform::IDENTITY));
            shape = self.shapes.get(shape).next;
        }
        let dynamic = record.kind == BodyKind::Dynamic;
        let mass = if dynamic {
            let combined = MassData::combine(parts);
            match record.mass {
                MassSpec::FromShapes => combined,
                MassSpec::Explicit(data) => data,
                MassSpec::Total(total) if combined.mass > 0.0 => {
                    let scale = total / combined.mass;
                    MassData {
                        mass: total,
                        center: combined.center,
                        inertia: combined.inertia * scale,
                    }
                }
                MassSpec::Total(total) => MassData {
                    mass: total,
                    center: Vec3::ZERO,
                    inertia: rikka_physics_math::Mat3::from_diagonal(Vec3::splat(
                        total * 0.4 * 0.25,
                    )),
                },
            }
        } else {
            MassData::ZERO
        };
        let set = self.sets.get_mut(record.set);
        let local = record.local as usize;
        let pose = &mut set.poses[local];
        let motion = &mut set.motion[local];
        motion.mass = mass.mass;
        motion.inv_mass = if mass.mass > 0.0 {
            1.0 / mass.mass
        } else {
            0.0
        };
        let locks = record.locks;
        let mut inertia = Sym3::from_mat3(mass.inertia);
        if locks & 0x38 == 0x38 || !dynamic || mass.mass <= 0.0 {
            inertia = Sym3::ZERO;
        }
        motion.inertia_local = inertia;
        motion.inv_inertia_local = if inertia == Sym3::ZERO {
            Sym3::ZERO
        } else {
            Sym3::from_mat3(inertia.to_mat3().inverse())
        };
        let local_center = if dynamic { mass.center } else { Vec3::ZERO };
        pose.local_center = local_center;
        pose.center = pose.transform.apply(local_center);
        pose.center0 = pose.center;
        pose.rotation0 = pose.transform.q;
        if local_bounds.is_valid() {
            let mut extent = 0.0f32;
            let mut reach = Vec3::ZERO;
            let mut buffer = ProxyBuffer::default();
            let mut shape = record.shape_head;
            while shape != NONE {
                let collider = &self.colliders[shape as usize];
                let (radius, axes) = shape_extent(&collider.shape, local_center, &mut buffer);
                extent = extent.max(radius);
                reach = reach.max(axes);
                shape = self.shapes.get(shape).next;
            }
            let set = self.sets.get_mut(record.set);
            let pose = &mut set.poses[local];
            (pose.extent, pose.reach) = if record.kind == BodyKind::Static {
                (0.0, Vec3::ZERO)
            } else {
                (extent, reach)
            };
            pose.min_extent = local_bounds.extent().min_element();
        } else {
            let pose = &mut self.sets.get_mut(record.set).poses[local];
            pose.extent = 0.0;
            pose.reach = Vec3::ZERO;
            pose.min_extent = 0.0;
        }
        let set = self.sets.get_mut(record.set);
        let rotation = set.poses[local].transform.q;
        set.motion[local].refresh_world_inertia(rotation);
        self.refresh_frame(body);
    }
}

fn shape_extent(shape: &Shape, center: Vec3, buffer: &mut ProxyBuffer) -> (f32, Vec3) {
    if let Some(proxy) = shape.proxy(buffer) {
        let (radius, reach) =
            proxy
                .points
                .iter()
                .fold((0.0f32, Vec3::ZERO), |(radius, reach), point| {
                    let offset = *point - center;
                    (radius.max(offset.length()), reach.max(offset.abs()))
                });
        return (radius + proxy.radius, reach + proxy.radius);
    }
    let bounds = shape.aabb(Transform::IDENTITY);
    let corners = [bounds.min, bounds.max];
    let mut extent = 0.0f32;
    for x in corners {
        for y in corners {
            for z in corners {
                extent = extent.max((Vec3::new(x.x, y.y, z.z) - center).length());
            }
        }
    }
    (
        extent,
        (bounds.min - center).abs().max((bounds.max - center).abs()),
    )
}
