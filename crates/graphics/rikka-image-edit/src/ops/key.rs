use bytemuck::{Pod, Zeroable};
use rikka_types::{Color, srgb};

use crate::error::Result;
use crate::ops::Effect;
use crate::ops::passes::working_plane;
use crate::picture::Frame;
use crate::shader::Shader;
use crate::work::{Io, PushHeader, Work};

const CHROMA: u32 = 0;
const LUMA: u32 = 1;
const CLIP: u32 = 2;

const CB_SCALE: f32 = 1.8556;
const CR_SCALE: f32 = 1.5748;

const CHROMA_REACH: f32 = 0.5;
const MAX_CLIPPED: f32 = 100_000.0;
const MAX_FEATHER: f32 = 1000.0;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct KeyPush {
    head: PushHeader,
    a: [f32; 4],
    b: [f32; 4],
}

fn keyed(work: &mut Work, frame: &Frame, mode: u32, a: [f32; 4], b: [f32; 4]) -> Result<Frame> {
    let out = working_plane(work, frame)?;
    let push = KeyPush {
        head: PushHeader::new(&frame.plane, &out)
            .mode(mode)
            .clamped(frame.depth.clamps()),
        a,
        b,
    };
    work.pass(Shader::Key, push, Io::new(&frame.plane, &out), 2.0)?;
    Ok(Frame {
        has_alpha: true,
        ..frame.with_plane(out)
    })
}

fn percent(value: f32) -> f32 {
    value.clamp(0.0, 100.0)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChromaKey {
    key: Color,
    tolerance: f32,
    softness: f32,
    spill: f32,
}

impl ChromaKey {
    #[must_use]
    pub fn new(key: Color) -> ChromaKey {
        ChromaKey {
            key,
            tolerance: 25.0,
            softness: 10.0,
            spill: 50.0,
        }
    }

    #[must_use]
    pub fn tolerance(mut self, tolerance: f32) -> ChromaKey {
        self.tolerance = percent(tolerance);
        self
    }

    #[must_use]
    pub fn softness(mut self, softness: f32) -> ChromaKey {
        self.softness = percent(softness);
        self
    }

    #[must_use]
    pub fn spill(mut self, spill: f32) -> ChromaKey {
        self.spill = percent(spill);
        self
    }
}

impl Effect for ChromaKey {
    fn apply(&self, work: &mut Work, frame: &Frame) -> Result<Frame> {
        let [r, g, b, _] = frame
            .space
            .translated([self.key.r, self.key.g, self.key.b, 1.0]);
        let y = srgb::luminance([r, g, b]);
        let key = [y, (b - y) / CB_SCALE, (r - y) / CR_SCALE, 0.0];
        let reach = [
            self.tolerance / 100.0 * CHROMA_REACH,
            self.softness / 100.0 * CHROMA_REACH,
            self.spill / 100.0,
            0.0,
        ];
        keyed(work, frame, CHROMA, key, reach)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Lumas {
    #[default]
    Bright,
    Dark,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LumaKey {
    threshold: f32,
    softness: f32,
    keeps: Lumas,
}

impl LumaKey {
    #[must_use]
    pub fn new(threshold: f32) -> LumaKey {
        LumaKey {
            threshold: percent(threshold),
            softness: 10.0,
            keeps: Lumas::Bright,
        }
    }

    #[must_use]
    pub fn softness(mut self, softness: f32) -> LumaKey {
        self.softness = percent(softness);
        self
    }

    #[must_use]
    pub fn keeps(mut self, keeps: Lumas) -> LumaKey {
        self.keeps = keeps;
        self
    }
}

impl Effect for LumaKey {
    fn apply(&self, work: &mut Work, frame: &Frame) -> Result<Frame> {
        let dark = match self.keeps {
            Lumas::Bright => 0.0,
            Lumas::Dark => 1.0,
        };
        let settings = [self.threshold / 100.0, self.softness / 100.0, dark, 0.0];
        keyed(work, frame, LUMA, settings, [0.0; 4])
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Clipping {
    left: f32,
    top: f32,
    right: f32,
    bottom: f32,
    feather: f32,
}

impl Clipping {
    #[must_use]
    pub fn new(left: f32, top: f32, right: f32, bottom: f32) -> Clipping {
        let band = |value: f32| value.clamp(0.0, MAX_CLIPPED);
        Clipping {
            left: band(left),
            top: band(top),
            right: band(right),
            bottom: band(bottom),
            feather: 0.0,
        }
    }

    #[must_use]
    pub fn feather(mut self, feather: f32) -> Clipping {
        self.feather = feather.clamp(0.0, MAX_FEATHER);
        self
    }
}

impl Effect for Clipping {
    fn apply(&self, work: &mut Work, frame: &Frame) -> Result<Frame> {
        let bands = [self.left, self.top, self.right, self.bottom];
        if bands.iter().all(|band| *band == 0.0) {
            return Ok(frame.clone());
        }
        keyed(work, frame, CLIP, bands, [self.feather, 0.0, 0.0, 0.0])
    }

    fn scaled(&self, factor: f32) -> Clipping {
        Clipping {
            left: self.left * factor,
            top: self.top * factor,
            right: self.right * factor,
            bottom: self.bottom * factor,
            feather: self.feather * factor,
        }
    }
}

crate::work::push_header!(KeyPush);
