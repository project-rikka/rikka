use rikka_physics_math::Vec3;
use rikka_physics_rigid::Character;

use super::World;
use crate::call::Call;
use crate::history::{Scope, Setup};
use crate::{CharacterDef, CharacterId, Error, Result};

impl World {
    pub fn create_character(&mut self, def: CharacterDef) -> Result<CharacterId> {
        let character = def.build()?;
        let id = self.characters.insert(character);
        self.log_setup(|| Setup::Character(def));
        Ok(id)
    }

    pub fn destroy_character(&mut self, id: CharacterId) -> Result<()> {
        self.characters
            .remove(id)
            .map(drop)
            .ok_or(Error::UnknownCharacter(id))?;
        self.log(Scope::Structure, || Call::DestroyCharacter(id));
        Ok(())
    }

    pub fn move_character(&mut self, id: CharacterId, velocity: Vec3, dt: f32) -> Result<()> {
        if !(velocity.is_finite() && dt.is_finite()) {
            return Err(Error::NotFinite {
                what: "character motion",
            });
        }
        let character = self
            .characters
            .get_mut(id)
            .ok_or(Error::UnknownCharacter(id))?;
        character.update(&self.rigid, velocity, dt);
        character.push_bodies(&mut self.rigid)?;
        self.log(Scope::Input, || Call::MoveCharacter(id, velocity, dt));
        Ok(())
    }

    pub fn set_character_position(&mut self, id: CharacterId, position: Vec3) -> Result<()> {
        if !position.is_finite() {
            return Err(Error::NotFinite {
                what: "character position",
            });
        }
        self.characters
            .get_mut(id)
            .ok_or(Error::UnknownCharacter(id))?
            .set_position(position);
        self.log(Scope::State, || Call::CharacterPosition(id, position));
        Ok(())
    }

    pub fn character(&self, id: CharacterId) -> Result<&Character> {
        self.characters.get(id).ok_or(Error::UnknownCharacter(id))
    }

    pub fn characters(&self) -> impl Iterator<Item = (CharacterId, &Character)> {
        self.characters.iter()
    }
}
