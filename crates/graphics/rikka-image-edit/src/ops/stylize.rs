use bytemuck::{Pod, Zeroable};

use crate::error::Result;
use crate::ops::Effect;
use crate::ops::passes::{Domain, arith, gaussian, in_premultiplied, premultiplied, straightened};
use crate::picture::Frame;
use crate::plane::{Plane, PlaneRef};
use crate::shader::Shader;
use crate::work::{Io, PushHeader, Work};

const EMBOSS: u32 = 0;
const EDGES: u32 = 1;
const LIGHTING: u32 = 2;
const VIGNETTE: u32 = 3;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct StylizePush {
    head: PushHeader,
    other: PlaneRef,
    a: [f32; 4],
    b: [f32; 4],
}

fn stylized(
    work: &mut Work,
    frame: &Frame,
    source: &Plane,
    other: &Plane,
    mode: u32,
    a: [f32; 4],
    b: [f32; 4],
) -> Result<Frame> {
    let out = work.plane(frame.extent(), frame.depth.working())?;
    let push = StylizePush {
        head: PushHeader::new(source, &out)
            .mode(mode)
            .clamped(frame.depth.clamps()),
        other: other.reference(),
        a,
        b,
    };
    work.pass(
        Shader::Stylize,
        push,
        Io::new(source, &out).read(other),
        9.0,
    )?;
    Ok(frame.with_plane(out))
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Emboss {
    angle: f32,
    height: f32,
    amount: f32,
}

impl Emboss {
    #[must_use]
    pub fn new(angle: f32, height: f32, amount: f32) -> Emboss {
        Emboss {
            angle,
            height: height.clamp(1.0, 100.0),
            amount: amount.clamp(1.0, 500.0),
        }
    }
}

impl Default for Emboss {
    fn default() -> Emboss {
        Emboss::new(135.0, 3.0, 100.0)
    }
}

impl Effect for Emboss {
    fn apply(&self, work: &mut Work, frame: &Frame) -> Result<Frame> {
        let (sin, cos) = self.angle.to_radians().sin_cos();
        let a = [cos, -sin, self.height, self.amount / 100.0];
        stylized(work, frame, &frame.plane, &frame.plane, EMBOSS, a, [0.0; 4])
    }

    fn scaled(&self, factor: f32) -> Emboss {
        Emboss {
            height: (self.height * factor).max(1.0),
            ..*self
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FindEdges;

impl Effect for FindEdges {
    fn apply(&self, work: &mut Work, frame: &Frame) -> Result<Frame> {
        stylized(
            work,
            frame,
            &frame.plane,
            &frame.plane,
            EDGES,
            [0.0; 4],
            [0.0; 4],
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vignette {
    amount: f32,
    midpoint: f32,
    roundness: f32,
    feather: f32,
    highlights: f32,
}

impl Vignette {
    #[must_use]
    pub fn new(amount: f32) -> Vignette {
        Vignette {
            amount: amount.clamp(-100.0, 100.0),
            midpoint: 50.0,
            roundness: 0.0,
            feather: 50.0,
            highlights: 0.0,
        }
    }

    #[must_use]
    pub fn midpoint(mut self, percent: f32) -> Vignette {
        self.midpoint = percent.clamp(0.0, 100.0);
        self
    }

    #[must_use]
    pub fn roundness(mut self, percent: f32) -> Vignette {
        self.roundness = percent.clamp(-100.0, 100.0);
        self
    }

    #[must_use]
    pub fn feather(mut self, percent: f32) -> Vignette {
        self.feather = percent.clamp(0.0, 100.0);
        self
    }

    #[must_use]
    pub fn highlights(mut self, percent: f32) -> Vignette {
        self.highlights = percent.clamp(0.0, 100.0);
        self
    }
}

impl Effect for Vignette {
    fn apply(&self, work: &mut Work, frame: &Frame) -> Result<Frame> {
        let a = [
            self.amount / 100.0,
            0.3 + self.midpoint / 100.0 * 0.9,
            self.roundness / 100.0,
            self.feather / 100.0,
        ];
        let b = [self.highlights / 100.0, 0.0, 0.0, 0.0];
        stylized(work, frame, &frame.plane, &frame.plane, VIGNETTE, a, b)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Glow {
    radius: f32,
    brightness: f32,
    threshold: f32,
}

impl Glow {
    #[must_use]
    pub fn new(radius: f32) -> Glow {
        Glow {
            radius: radius.clamp(0.0, 500.0),
            brightness: 1.0,
            threshold: 0.6,
        }
    }

    #[must_use]
    pub fn brightness(mut self, brightness: f32) -> Glow {
        self.brightness = brightness.clamp(0.0, 4.0);
        self
    }

    #[must_use]
    pub fn threshold(mut self, threshold: f32) -> Glow {
        self.threshold = threshold.clamp(0.0, 1.0);
        self
    }
}

impl Effect for Glow {
    fn apply(&self, work: &mut Work, frame: &Frame) -> Result<Frame> {
        in_premultiplied(work, frame, Domain::Linear, |work, linear| {
            let bright = arith(
                work,
                &linear,
                &linear,
                arith::BRIGHT,
                [self.threshold, 0.1, 0.0, 0.0],
                [0.0; 4],
            )?;
            let halo = gaussian(work, &bright, self.radius)?;
            let glowing = arith(
                work,
                &linear,
                &halo,
                arith::SCREEN,
                [self.brightness, 0.0, 0.0, 0.0],
                [0.0; 4],
            )?;
            Ok(glowing)
        })
    }

    fn scaled(&self, factor: f32) -> Glow {
        Glow {
            radius: self.radius * factor,
            ..*self
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct MosaicPush {
    head: PushHeader,
    cell: i32,
    pad: i32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mosaic {
    cell: f32,
}

impl Mosaic {
    #[must_use]
    pub fn new(cell: f32) -> Mosaic {
        Mosaic {
            cell: cell.clamp(2.0, 2000.0),
        }
    }
}

impl Effect for Mosaic {
    fn apply(&self, work: &mut Work, frame: &Frame) -> Result<Frame> {
        let cell = self.cell.round().max(1.0) as i32;
        let [width, height] = frame.extent();
        in_premultiplied(work, frame, Domain::Encoded, |work, encoded| {
            let cells = work.plane(
                [width.div_ceil(cell as u32), height.div_ceil(cell as u32)],
                encoded.format(),
            )?;
            let push = MosaicPush {
                head: PushHeader::new(&encoded, &cells).mode(0),
                cell,
                pad: 0,
            };
            work.pass(
                Shader::Mosaic,
                push,
                Io::new(&encoded, &cells),
                f64::from(cell * cell),
            )?;
            let expanded = work.plane(frame.extent(), encoded.format())?;
            let push = MosaicPush {
                head: PushHeader::new(&cells, &expanded).mode(1),
                cell,
                pad: 0,
            };
            work.pass(Shader::Mosaic, push, Io::new(&cells, &expanded), 1.0)?;
            Ok(expanded)
        })
    }

    fn scaled(&self, factor: f32) -> Mosaic {
        Mosaic {
            cell: (self.cell * factor).max(1.0),
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct KuwaharaPush {
    head: PushHeader,
    radius: f32,
    sharpness: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OilPaint {
    stylization: f32,
    cleanliness: f32,
    scale: f32,
    bristle_detail: f32,
    angle: f32,
    shine: f32,
}

impl Default for OilPaint {
    fn default() -> OilPaint {
        OilPaint {
            stylization: 2.5,
            cleanliness: 5.0,
            scale: 1.0,
            bristle_detail: 3.0,
            angle: -60.0,
            shine: 1.0,
        }
    }
}

impl OilPaint {
    #[must_use]
    pub fn new() -> OilPaint {
        OilPaint::default()
    }

    #[must_use]
    pub fn brush(
        mut self,
        stylization: f32,
        cleanliness: f32,
        scale: f32,
        bristle_detail: f32,
    ) -> OilPaint {
        self.stylization = stylization.clamp(0.1, 10.0);
        self.cleanliness = cleanliness.clamp(0.0, 10.0);
        self.scale = scale.clamp(0.1, 10.0);
        self.bristle_detail = bristle_detail.clamp(0.0, 10.0);
        self
    }

    #[must_use]
    pub fn lighting(mut self, angle: f32, shine: f32) -> OilPaint {
        self.angle = angle;
        self.shine = shine.clamp(0.0, 10.0);
        self
    }
}

impl Effect for OilPaint {
    fn apply(&self, work: &mut Work, frame: &Frame) -> Result<Frame> {
        let encoded = premultiplied(work, frame, Domain::Encoded)?;
        let painted = work.plane(frame.extent(), encoded.format())?;
        let radius = (self.stylization * self.scale * 1.5).max(1.0);
        let push = KuwaharaPush {
            head: PushHeader::new(&encoded, &painted),
            radius,
            sharpness: 2.0 + self.cleanliness * 1.4,
        };
        let taps = f64::from((2.0 * radius.ceil() + 1.0).powi(2)) * 8.0;
        work.pass(Shader::Kuwahara, push, Io::new(&encoded, &painted), taps)?;
        let straight = straightened(work, frame, &painted, Domain::Encoded)?;
        let (sin, cos) = self.angle.to_radians().sin_cos();
        let a = [cos, -sin, self.bristle_detail * 4.0, self.shine / 10.0];
        stylized(work, frame, &straight, &straight, LIGHTING, a, [0.0; 4])
    }

    fn scaled(&self, factor: f32) -> OilPaint {
        OilPaint {
            scale: (self.scale * factor).max(0.1),
            ..*self
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct TonalPush {
    head: PushHeader,
    base: PlaneRef,
    shadows: [f32; 4],
    highlights: [f32; 4],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShadowsHighlights {
    shadows: f32,
    highlights: f32,
    shadow_tone: f32,
    highlight_tone: f32,
    radius: f32,
    color: f32,
    midtones: f32,
}

impl ShadowsHighlights {
    #[must_use]
    pub fn new(shadows: f32, highlights: f32) -> ShadowsHighlights {
        ShadowsHighlights {
            shadows: shadows.clamp(0.0, 100.0),
            highlights: highlights.clamp(0.0, 100.0),
            shadow_tone: 50.0,
            highlight_tone: 50.0,
            radius: 30.0,
            color: 20.0,
            midtones: 0.0,
        }
    }

    #[must_use]
    pub fn tone(mut self, shadows: f32, highlights: f32) -> ShadowsHighlights {
        self.shadow_tone = shadows.clamp(0.0, 100.0);
        self.highlight_tone = highlights.clamp(0.0, 100.0);
        self
    }

    #[must_use]
    pub fn radius(mut self, radius: f32) -> ShadowsHighlights {
        self.radius = radius.clamp(0.0, 2500.0);
        self
    }

    #[must_use]
    pub fn color(mut self, color: f32) -> ShadowsHighlights {
        self.color = color.clamp(-100.0, 100.0);
        self
    }

    #[must_use]
    pub fn midtones(mut self, contrast: f32) -> ShadowsHighlights {
        self.midtones = contrast.clamp(-100.0, 100.0);
        self
    }
}

impl Effect for ShadowsHighlights {
    fn apply(&self, work: &mut Work, frame: &Frame) -> Result<Frame> {
        let encoded = premultiplied(work, frame, Domain::Encoded)?;
        let base = gaussian(work, &encoded, self.radius)?;
        let out = work.plane(frame.extent(), frame.depth.working())?;
        let push = TonalPush {
            head: PushHeader::new(&frame.plane, &out).clamped(frame.depth.clamps()),
            base: base.reference(),
            shadows: [
                self.shadows / 100.0,
                self.shadow_tone / 100.0,
                1.0 + self.color / 100.0,
                0.0,
            ],
            highlights: [
                self.highlights / 100.0,
                self.highlight_tone / 100.0,
                self.midtones / 100.0,
                0.0,
            ],
        };
        work.pass(
            Shader::Tonal,
            push,
            Io::new(&frame.plane, &out).read(&base),
            4.0,
        )?;
        Ok(frame.with_plane(out))
    }

    fn scaled(&self, factor: f32) -> ShadowsHighlights {
        ShadowsHighlights {
            radius: self.radius * factor,
            ..*self
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct DitherPush {
    head: PushHeader,
    error_buffer: u64,
    levels: u32,
    pad: u32,
}

const BAND: u64 = 256;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Dithering {
    Ordered,
    #[default]
    Diffusion,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Dither {
    levels: u32,
    method: Dithering,
}

impl Dither {
    #[must_use]
    pub fn new(levels: u32) -> Dither {
        Dither {
            levels: levels.clamp(2, 256),
            method: Dithering::default(),
        }
    }

    #[must_use]
    pub fn method(mut self, method: Dithering) -> Dither {
        self.method = method;
        self
    }
}

impl Effect for Dither {
    fn apply(&self, work: &mut Work, frame: &Frame) -> Result<Frame> {
        let [width, height] = frame.extent();
        let out = work.plane(frame.extent(), frame.depth.working())?;
        let error_buffer = match self.method {
            Dithering::Ordered => work.zeroed(4)?,
            Dithering::Diffusion => work.zeroed((BAND + 1) * u64::from(width) * 3 * 4)?,
        };
        let push = DitherPush {
            head: PushHeader::new(&frame.plane, &out).mode(self.method as u32),
            error_buffer: error_buffer.address(),
            levels: self.levels,
            pad: 0,
        };
        let io = Io::new(&frame.plane, &out).write_buffer(&error_buffer);
        let pixels = f64::from(width) * f64::from(height);
        let groups = match self.method {
            Dithering::Ordered => [width.div_ceil(256), height, 1],
            Dithering::Diffusion => [1, 1, 1],
        };
        work.groups(Shader::Dither, push, io, groups, pixels * 4.0)?;
        Ok(frame.with_plane(out))
    }
}

crate::work::push_header!(StylizePush, MosaicPush, KuwaharaPush, TonalPush);
