use rikka_physics_collision::{
    CollisionPlane, Material, Mover, QueryFilter, clip_vector, solve_planes,
};
use rikka_physics_math::{Plane, Vec3};

use crate::{BodyId, BodyKind, RayHit, Result, RigidWorld, ShapeId};

const ITERATIONS: u32 = 5;
const TOLERANCE: f32 = 0.01;
const BLOCKED: f32 = 0.9;
const MIN_TILT: f32 = 0.1;
const LEDGE_PROBE: f32 = 0.25;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CharacterContact {
    pub shape: ShapeId,
    pub body: BodyId,
    pub plane: Plane,
    pub point: Vec3,
    pub material: Material,
    pub supporting: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ground {
    pub shape: ShapeId,
    pub body: BodyId,
    pub point: Vec3,
    pub normal: Vec3,
    pub velocity: Vec3,
}

#[must_use]
#[derive(Clone, Debug)]
pub struct Character {
    mover: Mover,
    position: Vec3,
    velocity: Vec3,
    up: Vec3,
    max_slope: f32,
    step_height: f32,
    filter: QueryFilter,
    ground: Option<Ground>,
    contacts: Vec<CharacterContact>,
    planes: Vec<CollisionPlane>,
}

impl Character {
    pub fn new(mover: Mover, position: Vec3) -> Self {
        Self {
            mover,
            position,
            velocity: Vec3::ZERO,
            up: Vec3::Y,
            max_slope: 45f32.to_radians().cos(),
            step_height: 0.0,
            filter: QueryFilter::default(),
            ground: None,
            contacts: Vec::new(),
            planes: Vec::new(),
        }
    }

    pub fn capsule(radius: f32, height: f32, position: Vec3) -> Self {
        let half = (0.5 * height - radius).max(0.0);
        Self::new(
            Mover::new(
                Vec3::new(0.0, radius, 0.0),
                Vec3::new(0.0, radius + 2.0 * half, 0.0),
                radius,
            ),
            position,
        )
    }

    pub fn up(mut self, up: Vec3) -> Self {
        self.up = up.normalize_or(Vec3::Y);
        self
    }

    pub fn max_slope(mut self, radians: f32) -> Self {
        self.max_slope = radians.clamp(0.0, std::f32::consts::FRAC_PI_2).cos();
        self
    }

    pub fn step_height(mut self, height: f32) -> Self {
        self.step_height = height.max(0.0);
        self
    }

    pub fn filter(mut self, filter: QueryFilter) -> Self {
        self.filter = filter;
        self
    }

    pub fn position(&self) -> Vec3 {
        self.position
    }

    pub fn set_position(&mut self, position: Vec3) {
        self.position = position;
    }

    pub fn velocity(&self) -> Vec3 {
        self.velocity
    }

    pub fn mover(&self) -> Mover {
        self.mover.translated(self.position)
    }

    pub fn ground(&self) -> Option<&Ground> {
        self.ground.as_ref()
    }

    pub fn is_grounded(&self) -> bool {
        self.ground.is_some()
    }

    pub fn contacts(&self) -> &[CharacterContact] {
        &self.contacts
    }

    pub fn update(&mut self, world: &RigidWorld, velocity: Vec3, dt: f32) {
        if dt <= 0.0 || !velocity.is_finite() {
            return;
        }
        let platform = self.ground.map_or(Vec3::ZERO, |ground| ground.velocity);
        let desired = (velocity + platform) * dt;
        let start = self.position;
        let grounded = self.ground.is_some();
        let mut end = self.slide(world, start, desired);
        let horizontal_motion = self.horizontal(desired);
        if grounded && self.step_height > 0.0 && horizontal_motion.length() > TOLERANCE {
            let reached = self.horizontal(end - start).length();
            if reached < BLOCKED * horizontal_motion.length()
                && let Some(stepped) =
                    self.step_up(world, end, horizontal_motion - self.horizontal(end - start))
                && self.horizontal(stepped - start).length() > reached + TOLERANCE
            {
                end = stepped;
            }
        }
        if grounded && velocity.dot(self.up) <= 0.0 {
            end = self.stick(world, end);
        }
        self.position = end;
        self.collect(world, end);
        let clipped = clip_vector(velocity, &self.planes);
        self.velocity = clipped;
        self.ground = self.find_ground(world);
    }

    pub fn push_bodies(&self, world: &mut RigidWorld) -> Result<()> {
        for contact in &self.contacts {
            if world.body_kind(contact.body)? != BodyKind::Dynamic {
                continue;
            }
            let normal = -contact.plane.normal;
            let Some((effective, point_velocity)) =
                world.point_response(contact.body, contact.point, normal)
            else {
                continue;
            };
            if effective <= 0.0 {
                continue;
            }
            let impulse = (-(point_velocity - self.velocity).dot(normal) / effective).max(0.0);
            if impulse > 0.0 {
                world.apply_linear_impulse(contact.body, impulse * normal, contact.point)?;
            }
        }
        Ok(())
    }

    fn horizontal(&self, vector: Vec3) -> Vec3 {
        vector - vector.dot(self.up) * self.up
    }

    fn collect(&mut self, world: &RigidWorld, position: Vec3) {
        self.contacts.clear();
        self.planes.clear();
        let mover = self.mover.translated(position);
        let contacts = &mut self.contacts;
        world.collide_mover(&mover, self.filter, |shape, plane| {
            if let Ok(body) = world.shape_body(shape) {
                let material = world.material_at(shape, plane.material).unwrap_or_default();
                contacts.push(CharacterContact {
                    shape,
                    body,
                    plane: plane.plane,
                    point: plane.point,
                    material,
                    supporting: false,
                });
            }
            true
        });
        for index in 0..self.contacts.len() {
            let contact = self.contacts[index];
            let supporting = self.supports(world, contact.plane.normal, contact.point, position);
            self.contacts[index].supporting = supporting;
            let plane = if supporting {
                contact.plane
            } else {
                self.wall(contact.plane)
            };
            self.planes.push(CollisionPlane::rigid(plane));
        }
    }

    fn supports(&self, world: &RigidWorld, normal: Vec3, point: Vec3, position: Vec3) -> bool {
        let tilt = normal.dot(self.up);
        if tilt >= self.max_slope {
            return true;
        }
        if tilt <= 0.0 || (point - position).dot(self.up) - self.foot() > self.step_height {
            return false;
        }
        let reach = LEDGE_PROBE * self.mover.radius;
        let ahead = -self.horizontal(normal).normalize_or_zero() * reach;
        world
            .cast_ray_closest(
                point + ahead + self.up * reach,
                -self.up * (2.0 * reach),
                self.filter,
            )
            .is_some_and(|hit| hit.normal.dot(self.up) >= self.max_slope)
    }

    fn foot(&self) -> f32 {
        self.mover
            .center1
            .dot(self.up)
            .min(self.mover.center2.dot(self.up))
            - self.mover.radius
    }

    fn wall(&self, plane: Plane) -> Plane {
        if plane.normal.dot(self.up) <= 0.0 {
            return plane;
        }
        let wall = self.horizontal(plane.normal);
        let length = wall.length();
        if length <= f32::EPSILON {
            return plane;
        }
        let wall = wall / length;
        Plane::new(wall, plane.offset / plane.normal.dot(wall).max(MIN_TILT))
    }

    fn cast(&self, world: &RigidWorld, position: Vec3, delta: Vec3) -> Option<RayHit> {
        world.cast_mover(&self.mover.translated(position), delta, self.filter)
    }

    fn landing(&self, world: &RigidWorld, hit: &RayHit, position: Vec3) -> Option<Ground> {
        self.supports(world, hit.normal, hit.point, position)
            .then(|| Ground {
                shape: hit.shape,
                body: hit.body,
                point: hit.point,
                normal: hit.normal,
                velocity: world.point_velocity(hit.body, hit.point),
            })
    }

    fn slide(&mut self, world: &RigidWorld, start: Vec3, delta: Vec3) -> Vec3 {
        let target = start + delta;
        let mut position = start;
        for _ in 0..ITERATIONS {
            self.collect(world, position);
            let solution =
                solve_planes(target - position, &mut self.planes, world.config.tolerance);
            let fraction = self
                .cast(world, position, solution.delta)
                .map_or(1.0, |hit| hit.fraction);
            let step = solution.delta * fraction;
            position += step;
            if step.length_squared() < TOLERANCE * TOLERANCE {
                break;
            }
        }
        position
    }

    fn drop_onto(&self, world: &RigidWorld, position: Vec3, depth: f32) -> Option<Vec3> {
        let drop = -self.up * depth;
        let hit = self.cast(world, position, drop)?;
        let landed = position + drop * hit.fraction;
        self.landing(world, &hit, landed).map(|_| landed)
    }

    fn step_up(&mut self, world: &RigidWorld, from: Vec3, remaining: Vec3) -> Option<Vec3> {
        let lift = self.up * self.step_height;
        let raised = from + lift * self.cast(world, from, lift).map_or(1.0, |hit| hit.fraction);
        let moved = self.slide(world, raised, remaining);
        self.drop_onto(world, moved, (moved - from).dot(self.up) + TOLERANCE)
    }

    fn stick(&self, world: &RigidWorld, position: Vec3) -> Vec3 {
        self.drop_onto(world, position, self.step_height.max(TOLERANCE))
            .unwrap_or(position)
    }

    fn find_ground(&self, world: &RigidWorld) -> Option<Ground> {
        self.contacts
            .iter()
            .filter(|contact| contact.supporting)
            .max_by(|a, b| {
                a.plane
                    .normal
                    .dot(self.up)
                    .total_cmp(&b.plane.normal.dot(self.up))
            })
            .map(|contact| Ground {
                shape: contact.shape,
                body: contact.body,
                point: contact.point,
                normal: contact.plane.normal,
                velocity: world.point_velocity(contact.body, contact.point),
            })
            .or_else(|| {
                let drop = -self.up * TOLERANCE;
                let hit = self.cast(world, self.position, drop)?;
                self.landing(world, &hit, self.position + drop * hit.fraction)
            })
    }
}
