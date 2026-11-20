use rikka_physics_collision::BodyFrame;
use rikka_physics_math::{Quat, Transform, Vec3};

use super::RigidWorld;
use crate::body::{
    BULLET, Body, BodyKind, BodyRecord, CHARACTER, FAST_ROTATION, NONE, SLEEP_ENABLED,
};
use crate::set::{AWAKE_SET, BodyMotion, BodyPose, FIRST_SLEEPING_SET, STATIC_SET};
use crate::solver::state::{BodyState, DYNAMIC, FAST_ROTATION as STATE_FAST_ROTATION};
use crate::{BodyId, Error, Result};

fn finite(what: &'static str, value: f32) -> Result<()> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(Error::NotFinite { what, value })
    }
}

fn finite_vec(what: &'static str, value: Vec3) -> Result<()> {
    value
        .to_array()
        .into_iter()
        .try_for_each(|v| finite(what, v))
}

fn finite_quat(what: &'static str, value: Quat) -> Result<()> {
    value
        .to_array()
        .into_iter()
        .try_for_each(|v| finite(what, v))
}

impl RigidWorld {
    pub(crate) fn body_index(&self, id: BodyId) -> Result<u32> {
        if self.bodies.is_alive(id.index, id.generation) {
            Ok(id.index)
        } else {
            Err(Error::UnknownBody(id))
        }
    }

    pub(crate) fn body_id(&self, index: u32) -> BodyId {
        BodyId {
            index,
            generation: self.bodies.generation(index),
        }
    }

    pub fn create_body(&mut self, def: Body) -> Result<BodyId> {
        finite_vec("body position", def.pose.p)?;
        finite_quat("body rotation", def.pose.q)?;
        finite_vec("linear velocity", def.linear_velocity)?;
        finite_vec("angular velocity", def.angular_velocity)?;
        finite("gravity scale", def.gravity_scale)?;
        for (what, value) in [
            ("linear damping", def.linear_damping),
            ("angular damping", def.angular_damping),
        ] {
            if !(value.is_finite() && value >= 0.0) {
                return Err(Error::OutOfRange { what, value });
            }
        }
        for shape in &def.shapes {
            if !shape.materials.iter().all(|m| m.is_valid()) {
                return Err(Error::InvalidMaterial);
            }
            if !(shape.density.is_finite() && shape.density >= 0.0) {
                return Err(Error::OutOfRange {
                    what: "density",
                    value: shape.density,
                });
            }
        }

        let pose = Transform::new(def.pose.p, def.pose.q.normalize());
        let set = if def.kind == BodyKind::Static {
            STATIC_SET
        } else {
            AWAKE_SET
        };
        let mut flags = 0;
        if def.bullet {
            flags |= BULLET;
        }
        if def.sleep && !def.character {
            flags |= SLEEP_ENABLED;
        }
        if def.character {
            flags |= CHARACTER;
        }
        if def.fast_rotation {
            flags |= FAST_ROTATION;
        }
        let (index, _) = self.bodies.insert(BodyRecord {
            kind: def.kind,
            set,
            local: 0,
            island: NONE,
            island_prev: NONE,
            island_next: NONE,
            shape_head: NONE,
            shape_count: 0,
            contact_head: NONE,
            contact_count: 0,
            joint_head: NONE,
            joint_count: 0,
            alive: true,
            flags,
            locks: def.locks.bits(),
            sleep_time: if def.awake {
                0.0
            } else {
                self.config.time_to_sleep
            },
            sleep_threshold: def
                .sleep_threshold
                .unwrap_or_else(|| self.config.sleep_threshold()),
            user_data: def.user_data,
            mass: def.mass,
            target: None,
            move_event: NONE,
        });
        if self.frames.len() <= index as usize {
            self.frames.resize(index as usize + 1, BodyFrame::default());
        }

        let mut state_flags = def.locks.bits();
        if def.kind == BodyKind::Dynamic {
            state_flags |= DYNAMIC;
        }
        if def.fast_rotation {
            state_flags |= STATE_FAST_ROTATION;
        }
        let state = if def.kind == BodyKind::Static {
            BodyState::IDENTITY
        } else {
            BodyState::new(def.linear_velocity, def.angular_velocity, state_flags)
        };
        let motion = BodyMotion {
            gravity_scale: def.gravity_scale,
            linear_damping: def.linear_damping,
            angular_damping: def.angular_damping,
            ..Default::default()
        };
        let local =
            self.sets
                .get_mut(set)
                .push_body(index, BodyPose::new(pose, Vec3::ZERO), motion, state);
        self.bodies.get_mut(index).local = local;
        if def.kind == BodyKind::Dynamic {
            self.create_island_for(index);
        }
        for shape in def.shapes {
            self.attach_shape(index, shape);
        }
        self.update_mass(index);
        self.refresh_frame(index);
        Ok(self.body_id(index))
    }

    pub fn destroy_body(&mut self, id: BodyId) -> Result<()> {
        let index = self.body_index(id)?;
        self.wake_body(index);
        while self.bodies.get(index).joint_head != NONE {
            let joint = self.bodies.get(index).joint_head >> 1;
            self.destroy_joint_internal(joint);
        }
        while self.bodies.get(index).contact_head != NONE {
            let contact = self.bodies.get(index).contact_head >> 1;
            self.destroy_contact(contact, false);
        }
        while self.bodies.get(index).shape_head != NONE {
            let shape = self.bodies.get(index).shape_head;
            self.detach_shape(shape);
        }
        self.remove_body_from_island(index);
        let record = *self.bodies.get(index);
        self.remove_from_set(index, record.set, record.local);
        self.bodies.get_mut(index).alive = false;
        self.bodies.remove(index);
        Ok(())
    }

    pub(crate) fn remove_from_set(&mut self, body: u32, set: u32, local: u32) {
        debug_assert_eq!(self.sets.get(set).bodies[local as usize], body);
        if let Some(moved) = self.sets.get_mut(set).remove_body(local) {
            self.bodies.get_mut(moved).local = local;
            if set == AWAKE_SET {
                self.refresh_solver_indices(moved);
            }
        }
    }

    pub(crate) fn solver_index(&self, body: u32) -> u32 {
        let record = self.bodies.get(body);
        if record.set == AWAKE_SET {
            record.local + 1
        } else {
            0
        }
    }

    pub(crate) fn refresh_solver_indices(&mut self, body: u32) {
        let solver = self.solver_index(body);
        let mut key = self.bodies.get(body).contact_head;
        while key != NONE {
            let contact = self.contacts.get_mut(key >> 1);
            if key & 1 == 0 {
                contact.solver_a = solver;
            } else {
                contact.solver_b = solver;
            }
            key = contact.edges[(key & 1) as usize].next;
        }
        let mut key = self.bodies.get(body).joint_head;
        while key != NONE {
            let joint = self.joints.get_mut(key >> 1);
            if key & 1 == 0 {
                joint.solver_a = solver;
            } else {
                joint.solver_b = solver;
            }
            key = joint.edges[(key & 1) as usize].next;
        }
    }

    pub(crate) fn refresh_frame(&mut self, body: u32) {
        let record = self.bodies.get(body);
        let pose = &self.sets.get(record.set).poses[record.local as usize];
        let frame = &mut self.frames[body as usize];
        frame.transform = pose.transform;
        frame.center = pose.center;
        frame.reach = pose.reach;
    }

    pub(crate) fn pose_of(&self, body: u32) -> &BodyPose {
        let record = self.bodies.get(body);
        &self.sets.get(record.set).poses[record.local as usize]
    }

    pub(crate) fn state_of(&self, body: u32) -> &BodyState {
        let record = self.bodies.get(body);
        &self.sets.get(record.set).states[record.local as usize]
    }

    fn state_mut(&mut self, body: u32) -> &mut BodyState {
        let record = *self.bodies.get(body);
        &mut self.sets.get_mut(record.set).states[record.local as usize]
    }

    pub(crate) fn wake_body(&mut self, body: u32) {
        let set = self.bodies.get(body).set;
        if set >= FIRST_SLEEPING_SET {
            self.wake_set(set);
        }
        let island = self.bodies.get(body).island;
        let record = self.bodies.get_mut(body);
        record.sleep_time = 0.0;
        if island != NONE {
            let mut key = self.islands.get(island).bodies.head;
            while key != NONE {
                let member = self.bodies.get_mut(key);
                member.sleep_time = 0.0;
                key = member.island_next;
            }
        }
    }

    pub fn body_kind(&self, id: BodyId) -> Result<BodyKind> {
        Ok(self.bodies.get(self.body_index(id)?).kind)
    }

    pub fn pose(&self, id: BodyId) -> Result<Transform> {
        Ok(self.pose_of(self.body_index(id)?).transform)
    }

    pub fn center_of_mass(&self, id: BodyId) -> Result<Vec3> {
        Ok(self.pose_of(self.body_index(id)?).center)
    }

    pub fn linear_velocity(&self, id: BodyId) -> Result<Vec3> {
        Ok(self.state_of(self.body_index(id)?).v)
    }

    pub fn angular_velocity(&self, id: BodyId) -> Result<Vec3> {
        Ok(self.state_of(self.body_index(id)?).w)
    }

    pub fn mass(&self, id: BodyId) -> Result<f32> {
        let index = self.body_index(id)?;
        let record = self.bodies.get(index);
        Ok(self.sets.get(record.set).motion[record.local as usize].mass)
    }

    pub fn user_data(&self, id: BodyId) -> Result<u64> {
        Ok(self.bodies.get(self.body_index(id)?).user_data)
    }

    pub fn is_awake(&self, id: BodyId) -> Result<bool> {
        Ok(self.bodies.get(self.body_index(id)?).set == AWAKE_SET)
    }

    pub fn wake(&mut self, id: BodyId) -> Result<()> {
        let index = self.body_index(id)?;
        self.wake_body(index);
        Ok(())
    }

    pub fn set_linear_velocity(&mut self, id: BodyId, velocity: Vec3) -> Result<()> {
        finite_vec("linear velocity", velocity)?;
        let index = self.body_index(id)?;
        if self.bodies.get(index).kind == BodyKind::Static {
            return Ok(());
        }
        self.wake_body(index);
        self.state_mut(index).v = velocity;
        Ok(())
    }

    pub fn set_angular_velocity(&mut self, id: BodyId, velocity: Vec3) -> Result<()> {
        finite_vec("angular velocity", velocity)?;
        let index = self.body_index(id)?;
        if self.bodies.get(index).kind == BodyKind::Static {
            return Ok(());
        }
        self.wake_body(index);
        self.state_mut(index).w = velocity;
        Ok(())
    }

    pub fn apply_force(&mut self, id: BodyId, force: Vec3, point: Vec3) -> Result<()> {
        finite_vec("force", force)?;
        finite_vec("point", point)?;
        let index = self.body_index(id)?;
        if self.bodies.get(index).kind != BodyKind::Dynamic {
            return Ok(());
        }
        self.wake_body(index);
        let record = *self.bodies.get(index);
        let set = self.sets.get_mut(record.set);
        let center = set.poses[record.local as usize].center;
        let motion = &mut set.motion[record.local as usize];
        motion.force += force;
        motion.torque += (point - center).cross(force);
        Ok(())
    }

    pub fn apply_torque(&mut self, id: BodyId, torque: Vec3) -> Result<()> {
        finite_vec("torque", torque)?;
        let index = self.body_index(id)?;
        if self.bodies.get(index).kind != BodyKind::Dynamic {
            return Ok(());
        }
        self.wake_body(index);
        let record = *self.bodies.get(index);
        self.sets.get_mut(record.set).motion[record.local as usize].torque += torque;
        Ok(())
    }

    pub fn apply_linear_impulse(&mut self, id: BodyId, impulse: Vec3, point: Vec3) -> Result<()> {
        finite_vec("impulse", impulse)?;
        finite_vec("point", point)?;
        let index = self.body_index(id)?;
        if self.bodies.get(index).kind != BodyKind::Dynamic {
            return Ok(());
        }
        self.wake_body(index);
        let record = *self.bodies.get(index);
        let set = self.sets.get_mut(record.set);
        let local = record.local as usize;
        let center = set.poses[local].center;
        let motion = set.motion[local];
        let state = &mut set.states[local];
        state.v += impulse * motion.inv_mass;
        state.w += motion.inv_inertia.mul((point - center).cross(impulse));
        Ok(())
    }

    pub fn apply_angular_impulse(&mut self, id: BodyId, impulse: Vec3) -> Result<()> {
        finite_vec("impulse", impulse)?;
        let index = self.body_index(id)?;
        if self.bodies.get(index).kind != BodyKind::Dynamic {
            return Ok(());
        }
        self.wake_body(index);
        let record = *self.bodies.get(index);
        let set = self.sets.get_mut(record.set);
        let local = record.local as usize;
        let motion = set.motion[local];
        set.states[local].w += motion.inv_inertia.mul(impulse);
        Ok(())
    }

    pub fn move_to(&mut self, id: BodyId, target: Transform) -> Result<()> {
        let index = self.body_index(id)?;
        if self.bodies.get(index).kind != BodyKind::Kinematic {
            return Err(Error::NotKinematic(id));
        }
        finite_vec("target position", target.p)?;
        finite_quat("target rotation", target.q)?;
        self.wake_body(index);
        self.bodies.get_mut(index).target = Some(Transform::new(target.p, target.q.normalize()));
        self.solver.targets.push(index);
        Ok(())
    }

    pub fn set_pose(&mut self, id: BodyId, pose: Transform) -> Result<()> {
        let index = self.body_index(id)?;
        finite_vec("position", pose.p)?;
        let pose = Transform::new(pose.p, pose.q.normalize());
        self.wake_body(index);
        let record = *self.bodies.get(index);
        let set = self.sets.get_mut(record.set);
        let local = record.local as usize;
        let body = &mut set.poses[local];
        body.transform = pose;
        body.center = pose.apply(body.local_center);
        body.center0 = body.center;
        body.rotation0 = pose.q;
        set.motion[local].refresh_world_inertia(pose.q);
        self.bodies.get_mut(index).target = None;
        self.refresh_frame(index);
        self.refresh_proxies(index, true);
        Ok(())
    }
}
