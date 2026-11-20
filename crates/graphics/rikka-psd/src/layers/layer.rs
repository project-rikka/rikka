use crate::blend::BlendMode;
use crate::compression::Compression;
use crate::descriptor::Descriptor;
use crate::error::{Error, Result};
use crate::geometry::Rect;
use crate::info::{Info, Section, SectionKind};
use crate::key::Key;
use crate::layers::channel::{Channel, ChannelId};
use crate::layers::mask::{BlendingRanges, LayerMask};
use crate::pixels::{Depth, Plane};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LayerFlags(pub u8);

impl Default for LayerFlags {
    fn default() -> Self {
        LayerFlags(0x08)
    }
}

impl LayerFlags {
    pub fn transparency_protected(self) -> bool {
        self.0 & 0x01 != 0
    }

    pub fn hidden(self) -> bool {
        self.0 & 0x02 != 0
    }

    pub fn pixel_data_irrelevant(self) -> bool {
        self.0 & 0x08 != 0 && self.0 & 0x10 != 0
    }

    fn set(self, bit: u8, on: bool) -> Self {
        LayerFlags(if on { self.0 | bit } else { self.0 & !bit })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Layer {
    pub legacy_name: String,
    pub bounds: Rect,
    pub channels: Vec<Channel>,
    pub blend_mode: BlendMode,
    pub opacity: u8,
    pub clipping: bool,
    pub flags: LayerFlags,
    pub mask: Option<LayerMask>,
    pub blending_ranges: BlendingRanges,
    pub info: Vec<Info>,
}

impl Default for Layer {
    fn default() -> Self {
        Self {
            legacy_name: String::new(),
            bounds: Rect::EMPTY,
            channels: Vec::new(),
            blend_mode: BlendMode::Normal,
            opacity: 255,
            clipping: false,
            flags: LayerFlags::default(),
            mask: None,
            blending_ranges: BlendingRanges::default(),
            info: Vec::new(),
        }
    }
}

impl Layer {
    pub fn new(name: impl Into<String>) -> Self {
        let mut layer = Layer::default();
        layer.set_name(name);
        layer
    }

    pub fn name(&self) -> &str {
        self.info
            .iter()
            .find_map(|info| match info {
                Info::UnicodeName(name) => Some(name.as_str()),
                _ => None,
            })
            .unwrap_or(&self.legacy_name)
    }

    pub fn set_name(&mut self, name: impl Into<String>) {
        let name = name.into();
        let encoded = crate::io::mac_roman::encode(&name);
        self.legacy_name = crate::io::mac_roman::decode(&encoded[..encoded.len().min(255)]);
        self.set_info(Info::UnicodeName(name));
    }

    pub fn id(&self) -> Option<u32> {
        self.info.iter().find_map(|info| match info {
            Info::Id(id) => Some(*id),
            _ => None,
        })
    }

    pub fn visible(&self) -> bool {
        !self.flags.hidden()
    }

    pub fn set_visible(&mut self, visible: bool) {
        self.flags = self.flags.set(0x02, !visible);
    }

    pub fn transparency_locked(&self) -> bool {
        self.flags.transparency_protected()
    }

    pub fn set_transparency_locked(&mut self, locked: bool) {
        self.flags = self.flags.set(0x01, locked);
    }

    pub fn opacity(&self) -> f32 {
        f32::from(self.opacity) / 255.0
    }

    pub fn fill_opacity(&self) -> f32 {
        self.info
            .iter()
            .find_map(|info| match info {
                Info::FillOpacity(value) => Some(f32::from(*value) / 255.0),
                _ => None,
            })
            .unwrap_or(1.0)
    }

    pub fn set_fill_opacity(&mut self, opacity: f32) {
        self.set_info(Info::FillOpacity(
            (opacity.clamp(0.0, 1.0) * 255.0).round() as u8
        ));
    }

    pub fn section(&self) -> Option<&Section> {
        self.info.iter().find_map(|info| match info {
            Info::Section(section) => Some(section),
            _ => None,
        })
    }

    pub fn section_kind(&self) -> SectionKind {
        self.section()
            .map_or(SectionKind::Other, |section| section.kind)
    }

    pub fn find(&self, key: &[u8; 4]) -> Option<&Info> {
        self.info.iter().find(|info| info.key().0 == *key)
    }

    pub fn descriptor(&self, keys: &[&[u8; 4]]) -> Option<&Descriptor> {
        self.info.iter().find_map(|info| match info {
            Info::Descriptor(block) if keys.contains(&&block.key.0) => Some(&block.descriptor),
            _ => None,
        })
    }

    pub fn find_mut(&mut self, key: &[u8; 4]) -> Option<&mut Info> {
        self.info.iter_mut().find(|info| info.key().0 == *key)
    }

    pub fn set_info(&mut self, info: Info) {
        let key = info.key();
        match self.info.iter_mut().find(|entry| entry.key() == key) {
            Some(slot) => *slot = info,
            None => self.info.push(info),
        }
    }

    pub fn remove_info(&mut self, key: &[u8; 4]) -> Option<Info> {
        let index = self.info.iter().position(|info| info.key() == Key(*key))?;
        Some(self.info.remove(index))
    }

    pub fn channel(&self, id: ChannelId) -> Option<&Channel> {
        self.channels.iter().find(|channel| channel.id == id)
    }

    pub fn channel_bounds(&self, id: ChannelId) -> Rect {
        match id {
            ChannelId::UserMask => self.mask.map_or(Rect::EMPTY, |mask| mask.bounds),
            ChannelId::RealUserMask => self
                .mask
                .and_then(|mask| mask.real)
                .map_or(Rect::EMPTY, |real| real.bounds),
            _ => self.bounds,
        }
    }

    pub fn decode_channel(&self, id: ChannelId, depth: Depth) -> Result<Option<Plane>> {
        let Some(channel) = self.channel(id) else {
            return Ok(None);
        };
        let bounds = self.channel_bounds(id);
        if bounds.is_empty() {
            return Ok(None);
        }
        channel
            .data
            .decode(bounds.width(), bounds.height(), depth)
            .map(Some)
    }

    pub fn set_channel(
        &mut self,
        id: ChannelId,
        plane: &Plane,
        compression: Compression,
    ) -> Result<()> {
        let bounds = self.channel_bounds(id);
        if plane.width() != bounds.width() || plane.height() != bounds.height() {
            return Err(Error::Planes("the plane does not match the channel bounds"));
        }
        self.put_channel(Channel::new(id, plane, compression));
        Ok(())
    }

    pub fn put_channel(&mut self, channel: Channel) {
        match self
            .channels
            .iter_mut()
            .find(|current| current.id == channel.id)
        {
            Some(slot) => *slot = channel,
            None => {
                self.channels.push(channel);
                self.channels.sort_by_key(|channel| order(channel.id));
            }
        }
    }

    pub fn remove_channel(&mut self, id: ChannelId) {
        self.channels.retain(|channel| channel.id != id);
    }

    pub fn set_pixels(
        &mut self,
        bounds: Rect,
        planes: &[(ChannelId, Plane)],
        compression: Compression,
    ) -> Result<()> {
        self.bounds = bounds;
        self.channels
            .retain(|channel| matches!(channel.id, ChannelId::UserMask | ChannelId::RealUserMask));
        for (id, plane) in planes {
            self.set_channel(*id, plane, compression)?;
        }
        Ok(())
    }

    pub fn set_mask(
        &mut self,
        mask: LayerMask,
        plane: &Plane,
        compression: Compression,
    ) -> Result<()> {
        self.mask = Some(mask);
        self.set_channel(ChannelId::UserMask, plane, compression)
    }

    pub fn clear_mask(&mut self) {
        self.mask = None;
        self.remove_channel(ChannelId::UserMask);
        self.remove_channel(ChannelId::RealUserMask);
    }
}

fn order(id: ChannelId) -> i32 {
    match id {
        ChannelId::Transparency => -10,
        ChannelId::Color(index) => i32::from(index),
        ChannelId::UserMask => 1000,
        ChannelId::RealUserMask => 1001,
        ChannelId::Other(code) => 2000 - i32::from(code),
    }
}
