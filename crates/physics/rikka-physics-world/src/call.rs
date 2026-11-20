use rikka_physics_collision::{Filter, Material};
use rikka_physics_math::{Transform, Vec3};
use rikka_physics_rigid::{BodyId, JointId, ShapeId};

use crate::CharacterId;

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Call {
    Advance(f64),
    AdvanceTo(f64),
    MoveTo(BodyId, Transform),
    SetPose(BodyId, Transform),
    LinearVelocity(BodyId, Vec3),
    AngularVelocity(BodyId, Vec3),
    Force(BodyId, Vec3, Vec3),
    Torque(BodyId, Vec3),
    LinearImpulse(BodyId, Vec3, Vec3),
    AngularImpulse(BodyId, Vec3),
    Wake(BodyId),
    Gravity(Vec3),
    ShiftOrigin(Vec3),
    DestroyBody(BodyId),
    DestroyShape(ShapeId),
    DestroyJoint(JointId),
    Material(ShapeId, Material),
    ShapeFilter(ShapeId, Filter),
    Rebuild,
    Restart(f64),
    Snap,
    MoveCharacter(CharacterId, Vec3, f32),
    DestroyCharacter(CharacterId),
    CharacterPosition(CharacterId, Vec3),
}
