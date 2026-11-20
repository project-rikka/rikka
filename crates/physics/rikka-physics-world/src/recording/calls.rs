use rikka_file_format::{Codec, Reader, Writer};

use crate::call::Call;
use crate::{Error, Result};

impl Codec for Call {
    type Error = Error;

    fn encode(&self, out: &mut Writer<'_>) {
        match self {
            Call::Advance(dt) => {
                out.tag(0);
                out.put(dt);
            }
            Call::AdvanceTo(time) => {
                out.tag(1);
                out.put(time);
            }
            Call::MoveTo(body, pose) => {
                out.tag(2);
                out.put(&(*body, *pose));
            }
            Call::SetPose(body, pose) => {
                out.tag(3);
                out.put(&(*body, *pose));
            }
            Call::LinearVelocity(body, velocity) => {
                out.tag(4);
                out.put(&(*body, *velocity));
            }
            Call::AngularVelocity(body, velocity) => {
                out.tag(5);
                out.put(&(*body, *velocity));
            }
            Call::Force(body, force, point) => {
                out.tag(6);
                out.put(&(*body, (*force, *point)));
            }
            Call::Torque(body, torque) => {
                out.tag(7);
                out.put(&(*body, *torque));
            }
            Call::LinearImpulse(body, impulse, point) => {
                out.tag(8);
                out.put(&(*body, (*impulse, *point)));
            }
            Call::AngularImpulse(body, impulse) => {
                out.tag(9);
                out.put(&(*body, *impulse));
            }
            Call::Wake(body) => {
                out.tag(10);
                out.put(body);
            }
            Call::Gravity(gravity) => {
                out.tag(11);
                out.put(gravity);
            }
            Call::ShiftOrigin(offset) => {
                out.tag(12);
                out.put(offset);
            }
            Call::DestroyBody(body) => {
                out.tag(13);
                out.put(body);
            }
            Call::DestroyShape(shape) => {
                out.tag(14);
                out.put(shape);
            }
            Call::DestroyJoint(joint) => {
                out.tag(15);
                out.put(joint);
            }
            Call::Material(shape, material) => {
                out.tag(16);
                out.put(shape);
                out.put(material);
            }
            Call::ShapeFilter(shape, filter) => {
                out.tag(17);
                out.put(shape);
                out.put(filter);
            }
            Call::Rebuild => out.tag(18),
            Call::Restart(time) => {
                out.tag(19);
                out.put(time);
            }
            Call::Snap => out.tag(20),
            Call::MoveCharacter(character, velocity, dt) => {
                out.tag(21);
                out.put(character);
                out.put(velocity);
                out.put(dt);
            }
            Call::DestroyCharacter(character) => {
                out.tag(22);
                out.put(character);
            }
            Call::CharacterPosition(character, position) => {
                out.tag(23);
                out.put(character);
                out.put(position);
            }
        }
    }

    fn decode(input: &mut Reader<'_>) -> Result<Self> {
        Ok(match input.tag()? {
            0 => Call::Advance(input.get()?),
            1 => Call::AdvanceTo(input.get()?),
            2 => Call::MoveTo(input.get()?, input.get()?),
            3 => Call::SetPose(input.get()?, input.get()?),
            4 => Call::LinearVelocity(input.get()?, input.get()?),
            5 => Call::AngularVelocity(input.get()?, input.get()?),
            6 => Call::Force(input.get()?, input.get()?, input.get()?),
            7 => Call::Torque(input.get()?, input.get()?),
            8 => Call::LinearImpulse(input.get()?, input.get()?, input.get()?),
            9 => Call::AngularImpulse(input.get()?, input.get()?),
            10 => Call::Wake(input.get()?),
            11 => Call::Gravity(input.get()?),
            12 => Call::ShiftOrigin(input.get()?),
            13 => Call::DestroyBody(input.get()?),
            14 => Call::DestroyShape(input.get()?),
            15 => Call::DestroyJoint(input.get()?),
            16 => Call::Material(input.get()?, input.get()?),
            17 => Call::ShapeFilter(input.get()?, input.get()?),
            18 => Call::Rebuild,
            19 => Call::Restart(input.get()?),
            20 => Call::Snap,
            21 => Call::MoveCharacter(input.get()?, input.get()?, input.get()?),
            22 => Call::DestroyCharacter(input.get()?),
            23 => Call::CharacterPosition(input.get()?, input.get()?),
            _ => return Err(Error::MalformedRecording("unknown call")),
        })
    }
}
