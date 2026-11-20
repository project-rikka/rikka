use super::World;
use crate::call::Call;
use crate::recording::Recording;
use crate::{Error, Result};

impl World {
    pub fn start_recording(&mut self) -> Result<()> {
        if self.history.is_started() {
            return Err(Error::RecordingAfterStart);
        }
        self.recorder = Some(Recording::new(self.settings, self.history.entries()));
        Ok(())
    }

    pub fn finish_recording(&mut self) -> Option<Recording> {
        self.recorder.take()
    }

    pub fn is_recording(&self) -> bool {
        self.recorder.is_some()
    }

    pub(crate) fn replay_call(&mut self, call: &Call) -> Result<()> {
        let replaying = std::mem::replace(&mut self.replaying, true);
        let result = self.apply_call(call);
        self.replaying = replaying;
        result
    }

    pub(crate) fn tick_hashes(&self) -> &[u64] {
        &self.tick_hashes
    }

    pub(super) fn apply_call(&mut self, call: &Call) -> Result<()> {
        match call {
            Call::Advance(dt) => self.advance(*dt),
            Call::AdvanceTo(time) => self.advance_to(*time),
            Call::MoveTo(body, pose) => self.move_to(*body, *pose)?,
            Call::SetPose(body, pose) => self.set_pose(*body, *pose)?,
            Call::LinearVelocity(body, v) => self.set_linear_velocity(*body, *v)?,
            Call::AngularVelocity(body, v) => self.set_angular_velocity(*body, *v)?,
            Call::Force(body, force, point) => self.apply_force(*body, *force, *point)?,
            Call::Torque(body, torque) => self.apply_torque(*body, *torque)?,
            Call::LinearImpulse(body, impulse, point) => {
                self.apply_linear_impulse(*body, *impulse, *point)?
            }
            Call::AngularImpulse(body, impulse) => self.apply_angular_impulse(*body, *impulse)?,
            Call::Wake(body) => self.wake(*body)?,
            Call::Gravity(gravity) => self.set_gravity(*gravity)?,
            Call::ShiftOrigin(offset) => self.shift_origin(*offset)?,
            Call::DestroyBody(body) => self.destroy_body(*body)?,
            Call::DestroyShape(shape) => self.destroy_shape(*shape)?,
            Call::DestroyJoint(joint) => self.destroy_joint(*joint)?,
            Call::Material(shape, material) => self.set_material(*shape, *material)?,
            Call::ShapeFilter(shape, filter) => self.set_filter(*shape, *filter)?,
            Call::Rebuild => self.reset()?,
            Call::Restart(time) => self.restart(*time),
            Call::Snap => self.snap(),
            Call::MoveCharacter(character, velocity, dt) => {
                self.move_character(*character, *velocity, *dt)?
            }
            Call::DestroyCharacter(character) => self.destroy_character(*character)?,
            Call::CharacterPosition(character, position) => {
                self.set_character_position(*character, *position)?
            }
        }
        Ok(())
    }
}
