use crate::sample_format::SampleFormat;
use crate::tpdf::Tpdf;

#[derive(Debug, Clone)]
pub struct Quantizer {
    format: SampleFormat,
    bits: u32,
    dither: Option<Tpdf>,
}

impl Quantizer {
    #[must_use]
    pub fn new(format: SampleFormat) -> Quantizer {
        assert!(
            !format.is_dsd(),
            "{format} holds 1-bit DSD, not PCM samples"
        );
        Quantizer {
            format,
            bits: format.precision(),
            dither: None,
        }
    }

    #[must_use]
    pub fn significant_bits(mut self, bits: u32) -> Quantizer {
        if self.format.is_integer() {
            self.bits = bits.clamp(8, self.format.precision());
        }
        self
    }

    #[must_use]
    pub fn dithered(mut self, dithered: bool) -> Quantizer {
        self.set_dithered(dithered);
        self
    }

    pub fn set_dithered(&mut self, dithered: bool) {
        let needs_dither = dithered && self.format.is_integer();
        if needs_dither != self.dither.is_some() {
            self.dither = needs_dither.then(Tpdf::new);
        }
    }

    #[must_use]
    pub fn is_dithered(&self) -> bool {
        self.dither.is_some()
    }

    #[must_use]
    pub fn bits(&self) -> u32 {
        self.bits
    }

    #[must_use]
    pub fn format(&self) -> SampleFormat {
        self.format
    }

    pub fn write(&mut self, samples: &[f32], out: &mut Vec<u8>) {
        out.reserve(samples.len() * self.format.bytes());
        match self.format {
            SampleFormat::F32 => {
                for sample in samples {
                    out.extend_from_slice(&sample.to_le_bytes());
                }
            }
            SampleFormat::F64 => {
                for sample in samples {
                    out.extend_from_slice(&f64::from(*sample).to_le_bytes());
                }
            }
            _ => self.write_integers(samples, out),
        }
    }

    fn write_integers(&mut self, samples: &[f32], out: &mut Vec<u8>) {
        let container = self.format.precision();
        let shift = container - self.bits;
        let scale = f64::from(1_u32 << (self.bits - 1));
        let highest = scale - 1.0;
        for sample in samples {
            let mut level = f64::from(*sample) * scale;
            if let Some(dither) = &mut self.dither {
                level += dither.next();
            }
            let quantized = (level.round().clamp(-scale, highest) as i64) << shift;
            let bytes = (quantized as i32).to_le_bytes();
            match self.format {
                SampleFormat::S16 => out.extend_from_slice(&bytes[..2]),
                SampleFormat::S24Packed => out.extend_from_slice(&bytes[..3]),
                _ => out.extend_from_slice(&bytes),
            }
        }
    }
}
