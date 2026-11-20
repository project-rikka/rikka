use crate::error::Result;
use crate::io::{Reader, Writer};
use crate::key::Key;

#[derive(Debug, Clone, PartialEq)]
pub struct LegacyEffect {
    pub key: Key,
    pub body: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LegacyEffects {
    pub version: u16,
    pub effects: Vec<LegacyEffect>,
}

impl LegacyEffects {
    pub(crate) fn read(reader: &mut Reader<'_>) -> Result<Self> {
        let version = reader.u16()?;
        let count = reader.u16()?;
        let mut effects = Vec::with_capacity(usize::from(count));
        for _ in 0..count {
            reader.expect(b"8BIM")?;
            let key = reader.key()?;
            let size = reader.u32()?;
            let body = reader.bytes(size as usize)?.to_vec();
            effects.push(LegacyEffect { key, body });
        }
        Ok(Self { version, effects })
    }

    pub(crate) fn write(&self, writer: &mut Writer) {
        writer.u16(self.version);
        writer.u16(self.effects.len() as u16);
        for effect in &self.effects {
            writer.code(b"8BIM");
            writer.key(effect.key);
            writer.u32(effect.body.len() as u32);
            writer.bytes(&effect.body);
        }
    }
}
