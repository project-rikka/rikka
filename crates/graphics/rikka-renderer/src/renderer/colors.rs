use rikka_colorimetry::{BlendSpace, Description, Intent, Mapping};

use crate::renderer::Renderer;
use rikka_gpu::error::{Error, Result};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ColorId(pub(crate) u32);

impl ColorId {
    pub const SRGB: Self = Self(0);

    pub(crate) fn slot(self) -> u32 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum AlphaMode {
    #[default]
    Premultiplied,
    PremultipliedOptical,
    Straight,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ImageColor {
    pub description: Description,
    pub intent: Intent,
    pub alpha: AlphaMode,
}

impl Default for ImageColor {
    fn default() -> Self {
        Self {
            description: Description::SRGB,
            intent: Intent::Perceptual,
            alpha: AlphaMode::Premultiplied,
        }
    }
}

#[derive(Debug)]
struct Registered {
    color: ImageColor,
    holders: u32,
}

#[derive(Debug)]
pub struct Colors {
    slots: Vec<Option<Registered>>,
    revision: u64,
}

impl Default for Colors {
    fn default() -> Self {
        Self {
            slots: vec![Some(Registered {
                color: ImageColor::default(),
                holders: 1,
            })],
            revision: 0,
        }
    }
}

impl Colors {
    pub fn acquire(&mut self, color: ImageColor) -> Result<ColorId> {
        if color == ImageColor::default() {
            return Ok(ColorId::SRGB);
        }
        if let Some((slot, registered)) = self
            .slots
            .iter_mut()
            .enumerate()
            .find_map(|(slot, entry)| Some((slot, entry.as_mut()?)))
            .filter(|(_, registered)| registered.color == color)
        {
            registered.holders += 1;
            return Ok(ColorId(slot as u32));
        }
        if Mapping::new(&color.description, color.intent, &BlendSpace::Sdr).is_none() {
            return Err(Error::Invalid(
                "image colour has degenerate primaries".to_owned(),
            ));
        }
        let registered = Some(Registered { color, holders: 1 });
        self.revision += 1;
        match self.slots.iter().position(Option::is_none) {
            Some(slot) => {
                self.slots[slot] = registered;
                Ok(ColorId(slot as u32))
            }
            None => {
                self.slots.push(registered);
                Ok(ColorId((self.slots.len() - 1) as u32))
            }
        }
    }

    pub fn retain(&mut self, id: ColorId) {
        if let Some(Some(registered)) = self.slots.get_mut(id.0 as usize)
            && id != ColorId::SRGB
        {
            registered.holders += 1;
        }
    }

    pub fn release(&mut self, id: ColorId) {
        if id == ColorId::SRGB {
            return;
        }
        let Some(entry) = self.slots.get_mut(id.0 as usize) else {
            return;
        };
        let Some(registered) = entry else {
            return;
        };
        registered.holders -= 1;
        if registered.holders == 0 {
            *entry = None;
            self.revision += 1;
        }
    }

    pub(crate) fn revision(&self) -> u64 {
        self.revision
    }

    pub(crate) fn mappings(
        &self,
        space: &BlendSpace,
    ) -> impl Iterator<Item = Option<(Mapping, AlphaMode)>> {
        self.slots.iter().map(move |entry| {
            let registered = entry.as_ref()?;
            let color = registered.color;
            Some((
                Mapping::new(&color.description, color.intent, space)?,
                color.alpha,
            ))
        })
    }

    pub(crate) fn len(&self) -> usize {
        self.slots.len()
    }
}

impl Renderer {
    pub fn colors(&mut self) -> &mut Colors {
        &mut self.colors
    }
}
