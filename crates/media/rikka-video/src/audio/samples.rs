use rikka_ffmpeg::Frame;
use rikka_ffmpeg::avcodec::{self, AVSampleFormat};

const SIXTEEN: f32 = 32_768.0;
const TWENTY_FOUR: f32 = 8_388_608.0;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Layout {
    pub(crate) format: AVSampleFormat,
    pub(crate) depth: u32,
}

impl Layout {
    pub(crate) fn planar(self) -> bool {
        matches!(
            self.format,
            avcodec::AV_SAMPLE_FMT_U8P
                | avcodec::AV_SAMPLE_FMT_S16P
                | avcodec::AV_SAMPLE_FMT_S32P
                | avcodec::AV_SAMPLE_FMT_FLTP
                | avcodec::AV_SAMPLE_FMT_DBLP
                | avcodec::AV_SAMPLE_FMT_S64P
        )
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Quantizer {
    seed: u32,
}

impl Quantizer {
    pub(crate) fn new() -> Quantizer {
        Quantizer { seed: 0x9E37_79B9 }
    }

    pub(crate) fn fill(
        &mut self,
        layout: Layout,
        planes: &[*mut u8],
        channels: usize,
        interleaved: &[f32],
    ) {
        let frames = interleaved.len() / channels;
        for frame in 0..frames {
            for channel in 0..channels {
                let value = interleaved[frame * channels + channel];
                let (plane, slot) = match layout.planar() {
                    true => (planes[channel], frame),
                    false => (planes[0], frame * channels + channel),
                };
                unsafe { self.write(layout, plane, slot, value) };
            }
        }
    }

    unsafe fn write(&mut self, layout: Layout, plane: *mut u8, slot: usize, value: f32) {
        unsafe {
            match layout.format {
                avcodec::AV_SAMPLE_FMT_FLT | avcodec::AV_SAMPLE_FMT_FLTP => {
                    plane.cast::<f32>().add(slot).write(value);
                }
                avcodec::AV_SAMPLE_FMT_DBL | avcodec::AV_SAMPLE_FMT_DBLP => {
                    plane.cast::<f64>().add(slot).write(f64::from(value));
                }
                avcodec::AV_SAMPLE_FMT_S16 | avcodec::AV_SAMPLE_FMT_S16P => {
                    plane
                        .cast::<i16>()
                        .add(slot)
                        .write(self.quantize_i16(value));
                }
                avcodec::AV_SAMPLE_FMT_S32 | avcodec::AV_SAMPLE_FMT_S32P => {
                    plane
                        .cast::<i32>()
                        .add(slot)
                        .write(quantize_i32(value, layout.depth));
                }
                _ => plane.add(slot).write(quantize_u8(value)),
            }
        }
    }

    fn quantize_i16(&mut self, value: f32) -> i16 {
        let scaled = value * SIXTEEN;
        let shaped = match scaled.fract() == 0.0 {
            true => scaled,
            false => scaled + self.triangle(),
        };
        shaped.round().clamp(-SIXTEEN, SIXTEEN - 1.0) as i16
    }

    fn triangle(&mut self) -> f32 {
        self.uniform() - self.uniform()
    }

    fn uniform(&mut self) -> f32 {
        self.seed = self
            .seed
            .wrapping_mul(1_664_525)
            .wrapping_add(1_013_904_223);
        (self.seed >> 8) as f32 / (1u32 << 24) as f32
    }
}

fn quantize_i32(value: f32, depth: u32) -> i32 {
    if depth <= 24 {
        let scaled = (value * TWENTY_FOUR)
            .round()
            .clamp(-TWENTY_FOUR, TWENTY_FOUR - 1.0) as i32;
        return scaled << 8;
    }
    (f64::from(value) * 2_147_483_648.0)
        .round()
        .clamp(-2_147_483_648.0, 2_147_483_647.0) as i32
}

fn quantize_u8(value: f32) -> u8 {
    (value * 128.0 + 128.0).round().clamp(0.0, 255.0) as u8
}

pub(crate) fn append_decoded(frame: &Frame, output: &mut Vec<f32>) {
    let channels = frame.channels();
    let samples = frame.samples();
    if channels == 0 || samples == 0 {
        return;
    }
    let layout = Layout {
        format: frame.format(),
        depth: 32,
    };
    let planes = frame.extended_data();
    output.reserve(channels * samples);
    for sample in 0..samples {
        for channel in 0..channels {
            let (plane, slot) = match layout.planar() {
                true => (unsafe { *planes.add(channel) }, sample),
                false => (unsafe { *planes }, sample * channels + channel),
            };
            output.push(unsafe { read(layout.format, plane, slot) });
        }
    }
}

unsafe fn read(format: AVSampleFormat, plane: *const u8, slot: usize) -> f32 {
    unsafe {
        match format {
            avcodec::AV_SAMPLE_FMT_FLT | avcodec::AV_SAMPLE_FMT_FLTP => {
                plane.cast::<f32>().add(slot).read()
            }
            avcodec::AV_SAMPLE_FMT_DBL | avcodec::AV_SAMPLE_FMT_DBLP => {
                plane.cast::<f64>().add(slot).read() as f32
            }
            avcodec::AV_SAMPLE_FMT_S16 | avcodec::AV_SAMPLE_FMT_S16P => {
                f32::from(plane.cast::<i16>().add(slot).read()) / SIXTEEN
            }
            avcodec::AV_SAMPLE_FMT_S32 | avcodec::AV_SAMPLE_FMT_S32P => {
                (f64::from(plane.cast::<i32>().add(slot).read()) / 2_147_483_648.0) as f32
            }
            avcodec::AV_SAMPLE_FMT_S64 | avcodec::AV_SAMPLE_FMT_S64P => {
                (plane.cast::<i64>().add(slot).read() as f64 / 9_223_372_036_854_775_808.0) as f32
            }
            _ => (f32::from(plane.add(slot).read()) - 128.0) / 128.0,
        }
    }
}
