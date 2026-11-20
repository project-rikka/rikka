use std::collections::BTreeMap;

use crate::error::{Error, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum XSetting {
    Integer(i32),
    String(String),
    Color {
        red: u16,
        green: u16,
        blue: u16,
        alpha: u16,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Entry {
    value: XSetting,
    changed: u32,
}

#[derive(Debug, Default)]
pub(crate) struct XSettings {
    entries: BTreeMap<String, Entry>,
    serial: u32,
}

impl XSettings {
    pub(crate) fn set(&mut self, name: &str, value: XSetting) -> Result<bool> {
        if !valid_name(name) {
            return Err(Error::SettingName(name.to_owned()));
        }
        if self
            .entries
            .get(name)
            .is_some_and(|entry| entry.value == value)
        {
            return Ok(false);
        }
        self.entries.insert(
            name.to_owned(),
            Entry {
                value,
                changed: self.serial,
            },
        );
        Ok(true)
    }

    pub(crate) fn encode(&mut self) -> Vec<u8> {
        let mut data = Vec::new();
        data.push(u8::from(cfg!(target_endian = "big")));
        data.extend_from_slice(&[0; 3]);
        data.extend_from_slice(&self.serial.to_ne_bytes());
        data.extend_from_slice(&count(self.entries.len()).to_ne_bytes());
        for (name, entry) in &self.entries {
            data.push(match entry.value {
                XSetting::Integer(_) => 0,
                XSetting::String(_) => 1,
                XSetting::Color { .. } => 2,
            });
            data.push(0);
            data.extend_from_slice(&u16::try_from(name.len()).unwrap_or(u16::MAX).to_ne_bytes());
            padded(&mut data, name.as_bytes());
            data.extend_from_slice(&entry.changed.to_ne_bytes());
            match &entry.value {
                XSetting::Integer(value) => data.extend_from_slice(&value.to_ne_bytes()),
                XSetting::String(value) => {
                    data.extend_from_slice(&count(value.len()).to_ne_bytes());
                    padded(&mut data, value.as_bytes());
                }
                XSetting::Color {
                    red,
                    green,
                    blue,
                    alpha,
                } => {
                    for channel in [red, blue, green, alpha] {
                        data.extend_from_slice(&channel.to_ne_bytes());
                    }
                }
            }
        }
        self.serial = self.serial.wrapping_add(1);
        data
    }
}

fn count(length: usize) -> u32 {
    u32::try_from(length).unwrap_or(u32::MAX)
}

fn padded(data: &mut Vec<u8>, bytes: &[u8]) {
    data.extend_from_slice(bytes);
    data.resize(data.len() + (4 - bytes.len() % 4) % 4, 0);
}

fn valid_name(name: &str) -> bool {
    name.len() <= usize::from(u16::MAX)
        && !name.is_empty()
        && name.split('/').all(|part| {
            part.chars()
                .next()
                .is_some_and(|first| !first.is_ascii_digit())
                && part
                    .chars()
                    .all(|character| character.is_ascii_alphanumeric() || character == '_')
        })
}
