use rikka_physics_collision::{Filter, Material};
use rikka_physics_geometry::Shape;
use rikka_physics_math::{Transform, Vec3};
use rikka_physics_rigid::{
    Body, BodyId, BodyKind, ContactData, JointDef, JointEvent, JointId, ShapeDef, ShapeId,
};

use super::World;
use crate::call::Call;
use crate::history::{Scope, Setup};
use crate::{ContactEvent, Error, Result};

impl World {
    pub fn create_body(&mut self, body: Body) -> Result<BodyId> {
        let id = self.rigid.create_body(body.clone())?;
        let at = self.bodies.partition_point(|live| live.index < id.index);
        self.bodies.insert(at, id);
        self.poses.teleport(id, self.rigid.pose(id)?);
        self.log_setup(|| Setup::Body(body));
        Ok(id)
    }

    pub fn destroy_body(&mut self, id: BodyId) -> Result<()> {
        self.rigid.destroy_body(id)?;
        self.bodies.retain(|&live| live != id);
        let rigid = &self.rigid;
        self.joints
            .retain(|&joint| rigid.joint_bodies(joint).is_ok());
        self.targets.remove(id);
        self.poses.forget(id);
        self.log(Scope::Structure, || Call::DestroyBody(id));
        Ok(())
    }

    pub fn create_shape(&mut self, body: BodyId, shape: impl Into<ShapeDef>) -> Result<ShapeId> {
        let shape = shape.into();
        let id = self.rigid.create_shape(body, shape.clone())?;
        self.log_setup(|| Setup::Shape(body, shape));
        Ok(id)
    }

    pub fn destroy_shape(&mut self, id: ShapeId) -> Result<()> {
        self.rigid.destroy_shape(id)?;
        self.log(Scope::Structure, || Call::DestroyShape(id));
        Ok(())
    }

    pub fn create_joint(&mut self, joint: impl Into<JointDef>) -> Result<JointId> {
        let joint = joint.into();
        let id = self.rigid.create_joint(joint)?;
        self.joints.push(id);
        self.log_setup(|| Setup::Joint(Box::new(joint)));
        Ok(id)
    }

    pub fn destroy_joint(&mut self, id: JointId) -> Result<()> {
        self.rigid.destroy_joint(id)?;
        self.joints.retain(|&live| live != id);
        self.log(Scope::Structure, || Call::DestroyJoint(id));
        Ok(())
    }

    pub fn move_to(&mut self, id: BodyId, target: Transform) -> Result<()> {
        if self.rigid.body_kind(id)? != BodyKind::Kinematic {
            return Err(Error::NotKinematic(id));
        }
        if !target.is_finite() {
            return Err(Error::NotFinite {
                what: "target pose",
            });
        }
        let target = Transform::new(target.p, target.q.normalize());
        let current = self.rigid.pose(id)?;
        self.targets.set(id, current, target);
        self.log(Scope::Input, || Call::MoveTo(id, target));
        Ok(())
    }

    pub fn set_pose(&mut self, id: BodyId, pose: Transform) -> Result<()> {
        self.rigid.set_pose(id, pose)?;
        let pose = self.rigid.pose(id)?;
        self.targets.remove(id);
        self.poses.teleport(id, pose);
        self.log(Scope::State, || Call::SetPose(id, pose));
        Ok(())
    }

    pub fn pose(&self, id: BodyId) -> Result<Transform> {
        match self
            .poses
            .interpolated(id, self.clock.tick(), self.clock.alpha())
        {
            Some(pose) => Ok(pose),
            None => Ok(self.rigid.pose(id)?),
        }
    }

    pub fn latest_pose(&self, id: BodyId) -> Result<Transform> {
        Ok(self.rigid.pose(id)?)
    }

    pub fn body_kind(&self, id: BodyId) -> Result<BodyKind> {
        Ok(self.rigid.body_kind(id)?)
    }

    pub fn bodies(&self) -> &[BodyId] {
        &self.bodies
    }

    pub fn joints(&self) -> &[JointId] {
        &self.joints
    }

    pub fn center_of_mass(&self, id: BodyId) -> Result<Vec3> {
        Ok(self.rigid.center_of_mass(id)?)
    }

    pub fn mass(&self, id: BodyId) -> Result<f32> {
        Ok(self.rigid.mass(id)?)
    }

    pub fn user_data(&self, id: BodyId) -> Result<u64> {
        Ok(self.rigid.user_data(id)?)
    }

    pub fn is_awake(&self, id: BodyId) -> Result<bool> {
        Ok(self.rigid.is_awake(id)?)
    }

    pub fn wake(&mut self, id: BodyId) -> Result<()> {
        self.rigid.wake(id)?;
        self.log(Scope::State, || Call::Wake(id));
        Ok(())
    }

    pub fn linear_velocity(&self, id: BodyId) -> Result<Vec3> {
        Ok(self.rigid.linear_velocity(id)?)
    }

    pub fn angular_velocity(&self, id: BodyId) -> Result<Vec3> {
        Ok(self.rigid.angular_velocity(id)?)
    }

    pub fn set_linear_velocity(&mut self, id: BodyId, velocity: Vec3) -> Result<()> {
        self.rigid.set_linear_velocity(id, velocity)?;
        self.log(Scope::State, || Call::LinearVelocity(id, velocity));
        Ok(())
    }

    pub fn set_angular_velocity(&mut self, id: BodyId, velocity: Vec3) -> Result<()> {
        self.rigid.set_angular_velocity(id, velocity)?;
        self.log(Scope::State, || Call::AngularVelocity(id, velocity));
        Ok(())
    }

    pub fn apply_force(&mut self, id: BodyId, force: Vec3, point: Vec3) -> Result<()> {
        self.rigid.apply_force(id, force, point)?;
        self.log(Scope::State, || Call::Force(id, force, point));
        Ok(())
    }

    pub fn apply_torque(&mut self, id: BodyId, torque: Vec3) -> Result<()> {
        self.rigid.apply_torque(id, torque)?;
        self.log(Scope::State, || Call::Torque(id, torque));
        Ok(())
    }

    pub fn apply_linear_impulse(&mut self, id: BodyId, impulse: Vec3, point: Vec3) -> Result<()> {
        self.rigid.apply_linear_impulse(id, impulse, point)?;
        self.log(Scope::State, || Call::LinearImpulse(id, impulse, point));
        Ok(())
    }

    pub fn apply_angular_impulse(&mut self, id: BodyId, impulse: Vec3) -> Result<()> {
        self.rigid.apply_angular_impulse(id, impulse)?;
        self.log(Scope::State, || Call::AngularImpulse(id, impulse));
        Ok(())
    }

    pub fn shape(&self, id: ShapeId) -> Result<&Shape> {
        Ok(self.rigid.shape(id)?)
    }

    pub fn shape_body(&self, id: ShapeId) -> Result<BodyId> {
        Ok(self.rigid.shape_body(id)?)
    }

    pub fn set_material(&mut self, id: ShapeId, material: Material) -> Result<()> {
        self.rigid.set_material(id, material)?;
        self.log(Scope::Structure, || Call::Material(id, material));
        Ok(())
    }

    pub fn set_filter(&mut self, id: ShapeId, filter: Filter) -> Result<()> {
        self.rigid.set_filter(id, filter)?;
        self.log(Scope::Structure, || Call::ShapeFilter(id, filter));
        Ok(())
    }

    pub fn gravity(&self) -> Vec3 {
        self.gravity
    }

    pub fn set_gravity(&mut self, gravity: Vec3) -> Result<()> {
        if !gravity.is_finite() {
            return Err(Error::NotFinite { what: "gravity" });
        }
        self.gravity = gravity;
        self.rigid.set_gravity(gravity);
        self.log(Scope::Structure, || Call::Gravity(gravity));
        Ok(())
    }

    pub fn joint_reaction(&self, id: JointId) -> Result<(f32, f32)> {
        Ok(self.rigid.joint_reaction(id)?)
    }

    pub fn is_joint_broken(&self, id: JointId) -> Result<bool> {
        Ok(self.rigid.is_joint_broken(id)?)
    }

    pub fn contact_events(&self) -> &[ContactEvent] {
        &self.events.contacts
    }

    pub fn joint_events(&self) -> &[JointEvent] {
        &self.events.joints
    }

    pub fn body_contacts(&self, id: BodyId) -> Result<Vec<ContactData>> {
        Ok(self.rigid.body_contacts(id)?)
    }
}
