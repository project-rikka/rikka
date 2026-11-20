use rikka_physics_math::Vec3;

use super::World;
use crate::call::Call;
use crate::history::Scope;
use crate::{Error, Result};

impl World {
    pub fn shift_origin(&mut self, offset: Vec3) -> Result<()> {
        if !offset.is_finite() {
            return Err(Error::NotFinite {
                what: "origin offset",
            });
        }
        self.rigid.shift_origin(offset);
        self.targets.shift(offset);
        self.poses.shift(offset);
        for character in self.characters.values_mut() {
            character.set_position(character.position() - offset);
        }
        self.log(Scope::Structure, || Call::ShiftOrigin(offset));
        Ok(())
    }
}
