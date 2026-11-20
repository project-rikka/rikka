use super::RigidWorld;
use crate::body::{BodyKind, NONE};
use crate::graph::{Constraint, Edge};
use crate::joint::{Anchor, JointBase, JointDef, JointImpulses, JointKind, JointRecord};
use crate::set::{AWAKE_SET, DISABLED_SET, FIRST_SLEEPING_SET, swap_remove};
use crate::{BodyId, Error, JointId, Result};
use rikka_physics_math::Transform;

fn monitored(base: &crate::joint::JointBase) -> bool {
    base.break_force < f32::MAX
        || base.break_torque < f32::MAX
        || base.force_threshold < f32::MAX
        || base.torque_threshold < f32::MAX
}

impl RigidWorld {
    pub(crate) fn joint_index(&self, id: JointId) -> Result<u32> {
        if self.joints.is_alive(id.index, id.generation) {
            Ok(id.index)
        } else {
            Err(Error::UnknownJoint(id))
        }
    }

    pub(crate) fn joint_id(&self, index: u32) -> JointId {
        JointId {
            index,
            generation: self.joints.generation(index),
        }
    }

    pub fn create_joint(&mut self, def: impl Into<JointDef>) -> Result<JointId> {
        let def = def.into();
        let a = self.body_index(def.base.body_a)?;
        let b = self.body_index(def.base.body_b)?;
        if a == b {
            return Err(Error::SelfJoint);
        }
        if self.bodies.get(a).kind != BodyKind::Dynamic
            && self.bodies.get(b).kind != BodyKind::Dynamic
        {
            return Err(Error::NoDynamicBody);
        }
        if !def.params.is_valid() || def.base.softness.is_some_and(|s| !s.is_valid()) {
            return Err(Error::InvalidJoint);
        }
        let (frame_a, frame_b) = match def.base.anchor {
            Anchor::World(world) => (
                self.pose_of(a).transform.inverse_mul(&world),
                self.pose_of(b).transform.inverse_mul(&world),
            ),
            Anchor::Local { a, b } => (a, b),
        };
        if !frame_a.is_finite() || !frame_b.is_finite() {
            return Err(Error::InvalidJoint);
        }
        for body in [a, b] {
            let set = self.bodies.get(body).set;
            if set >= FIRST_SLEEPING_SET {
                self.wake_set(set);
            }
        }
        let head_a = self.bodies.get(a).joint_head;
        let head_b = self.bodies.get(b).joint_head;
        let (index, _) = self.joints.insert(JointRecord {
            params: def.params,
            frame_a,
            frame_b,
            body_a: a,
            body_b: b,
            edges: [
                Edge {
                    body: a,
                    prev: NONE,
                    next: head_a,
                },
                Edge {
                    body: b,
                    prev: NONE,
                    next: head_b,
                },
            ],
            solver_a: self.solver_index(a),
            solver_b: self.solver_index(b),
            set: AWAKE_SET,
            color: NONE,
            local: NONE,
            island: NONE,
            island_prev: NONE,
            island_next: NONE,
            impulses: JointImpulses::ZERO,
            reaction_scale: 0.0,
            softness: def.base.softness,
            collide_connected: def.base.collide_connected,
            break_force: def.base.break_force,
            break_torque: def.base.break_torque,
            force_threshold: def.base.force_threshold,
            torque_threshold: def.base.torque_threshold,
            over_threshold: false,
            user_data: def.base.user_data,
            revision: 0,
        });
        if monitored(&def.base) {
            self.monitored_joints += 1;
        }
        for (side, head) in [(0u32, head_a), (1, head_b)] {
            if head != NONE {
                self.joints.get_mut(head >> 1).edges[(head & 1) as usize].prev = index << 1 | side;
            }
        }
        self.bodies.get_mut(a).joint_head = index << 1;
        self.bodies.get_mut(a).joint_count += 1;
        self.bodies.get_mut(b).joint_head = index << 1 | 1;
        self.bodies.get_mut(b).joint_count += 1;
        self.add_joint_to_graph(index);
        self.link_joint(index);
        if !def.base.collide_connected {
            self.destroy_contacts_between(a, b);
        }
        Ok(self.joint_id(index))
    }

    fn destroy_contacts_between(&mut self, a: u32, b: u32) {
        let mut key = self.bodies.get(a).contact_head;
        while key != NONE {
            let id = key >> 1;
            let contact = *self.contacts.get(id);
            key = contact.edges[(key & 1) as usize].next;
            if contact.body_a() == b || contact.body_b() == b {
                self.destroy_contact(id, false);
            }
        }
    }

    pub(crate) fn add_joint_to_graph(&mut self, joint: u32) {
        let record = *self.joints.get(joint);
        let a = (
            record.body_a,
            self.bodies.get(record.body_a).kind == BodyKind::Dynamic,
        );
        let b = (
            record.body_b,
            self.bodies.get(record.body_b).kind == BodyKind::Dynamic,
        );
        let (color, local) = self.graph.add(Constraint::Joint, joint, a, b);
        let record = self.joints.get_mut(joint);
        record.color = color;
        record.local = local;
    }

    pub(crate) fn remove_joint_from_graph(&mut self, joint: u32) {
        let record = *self.joints.get(joint);
        if let Some(moved) = self.graph.remove(
            Constraint::Joint,
            record.color,
            record.local,
            record.body_a,
            record.body_b,
        ) {
            self.joints.get_mut(moved).local = record.local;
        }
        let record = self.joints.get_mut(joint);
        record.color = NONE;
        record.local = NONE;
    }

    fn detach_joint(&mut self, joint: u32) {
        let record = *self.joints.get(joint);
        if record.color != NONE {
            self.remove_joint_from_graph(joint);
        } else if record.set >= FIRST_SLEEPING_SET
            && let Some(moved) =
                swap_remove(&mut self.sets.get_mut(record.set).joints, record.local)
        {
            self.joints.get_mut(moved).local = record.local;
        }
        if record.island != NONE {
            self.unlink_joint(joint);
        }
    }

    pub(crate) fn destroy_joint_internal(&mut self, joint: u32) {
        let record = *self.joints.get(joint);
        for body in [record.body_a, record.body_b] {
            self.wake_body(body);
        }
        let record = *self.joints.get(joint);
        self.detach_joint(joint);
        for edge in record.edges {
            if edge.prev != NONE {
                self.joints.get_mut(edge.prev >> 1).edges[(edge.prev & 1) as usize].next =
                    edge.next;
            } else {
                self.bodies.get_mut(edge.body).joint_head = edge.next;
            }
            if edge.next != NONE {
                self.joints.get_mut(edge.next >> 1).edges[(edge.next & 1) as usize].prev =
                    edge.prev;
            }
            self.bodies.get_mut(edge.body).joint_count -= 1;
        }
        if record.break_force < f32::MAX
            || record.break_torque < f32::MAX
            || record.force_threshold < f32::MAX
            || record.torque_threshold < f32::MAX
        {
            self.monitored_joints -= 1;
        }
        self.joints.remove(joint);
        if !record.collide_connected {
            for shape in [record.body_a, record.body_b] {
                self.refresh_proxies(shape, true);
            }
        }
    }

    pub fn destroy_joint(&mut self, id: JointId) -> Result<()> {
        let index = self.joint_index(id)?;
        self.destroy_joint_internal(index);
        Ok(())
    }

    pub(crate) fn break_joint(&mut self, joint: u32) {
        self.detach_joint(joint);
        self.joints.get_mut(joint).set = DISABLED_SET;
    }

    pub fn is_joint_broken(&self, id: JointId) -> Result<bool> {
        Ok(self.joints.get(self.joint_index(id)?).set == DISABLED_SET)
    }

    pub fn joint_reaction(&self, id: JointId) -> Result<(f32, f32)> {
        Ok(self.joints.get(self.joint_index(id)?).reaction())
    }

    pub fn joint_user_data(&self, id: JointId) -> Result<u64> {
        Ok(self.joints.get(self.joint_index(id)?).user_data)
    }
}

impl RigidWorld {
    pub fn joint_bodies(&self, id: JointId) -> Result<(BodyId, BodyId)> {
        let record = self.joints.get(self.joint_index(id)?);
        Ok((self.body_id(record.body_a), self.body_id(record.body_b)))
    }

    pub fn joint_local_frames(&self, id: JointId) -> Result<(Transform, Transform)> {
        let record = self.joints.get(self.joint_index(id)?);
        Ok((record.frame_a, record.frame_b))
    }

    pub fn joint_kind(&self, id: JointId) -> Result<JointKind> {
        Ok(self.joints.get(self.joint_index(id)?).params.kind())
    }

    pub fn joint_def(&self, id: JointId) -> Result<JointDef> {
        let record = self.joints.get(self.joint_index(id)?);
        Ok(JointDef {
            base: JointBase {
                body_a: self.body_id(record.body_a),
                body_b: self.body_id(record.body_b),
                anchor: Anchor::Local {
                    a: record.frame_a,
                    b: record.frame_b,
                },
                collide_connected: record.collide_connected,
                break_force: record.break_force,
                break_torque: record.break_torque,
                force_threshold: record.force_threshold,
                torque_threshold: record.torque_threshold,
                softness: record.softness,
                user_data: record.user_data,
            },
            params: record.params,
        })
    }
}
