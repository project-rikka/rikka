use std::collections::HashMap;

use rikka_colorimetry::{BlendSpace, HdrSpace, Mastering, Primaries};
use rikka_compositor_kms::{Edid, HdrMetadata, Signal};
use rikka_compositor_surface::{NamedPrimaries, Parametric, Surfaces, TransferFunction};
use rikka_compositor_types::OutputId;

use crate::outputs::{Head, OutputConfig};

const UNKNOWN_PEAK: f64 = 400.0;
const PQ_SWING: f64 = 10_000.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct HdrDisplay {
    pub(crate) primaries: Primaries,
    pub(crate) peak: f64,
    pub(crate) frame_average: f64,
    pub(crate) black: f64,
}

impl HdrDisplay {
    pub(crate) fn from_edid(edid: &Edid) -> Option<Self> {
        let hdr = edid.hdr.filter(|hdr| hdr.pq)?;
        let peak = hdr.max_luminance.unwrap_or(UNKNOWN_PEAK);
        Some(Self {
            primaries: edid.primaries.unwrap_or(Primaries::BT2020),
            peak,
            frame_average: hdr.max_frame_average.unwrap_or(peak).min(peak),
            black: hdr.min_luminance.unwrap_or(0.0),
        })
    }

    fn metadata(&self) -> HdrMetadata {
        HdrMetadata {
            primaries: self.primaries,
            max_luminance: self.peak,
            min_luminance: self.black,
            max_cll: self.peak,
            max_fall: self.frame_average,
        }
    }
}

fn lit_hdr(head: &Head, config: &OutputConfig) -> Option<HdrDisplay> {
    head.hdr.filter(|_| config.hdr)
}

pub(crate) fn output_signal(head: &Head, config: &OutputConfig) -> Signal {
    match lit_hdr(head, config) {
        Some(display) => Signal::Pq(display.metadata()),
        None => Signal::Sdr,
    }
}

pub(crate) fn blend_space(head: &Head, config: &OutputConfig) -> BlendSpace {
    match lit_hdr(head, config) {
        Some(display) => BlendSpace::Hdr(HdrSpace {
            reference: config.sdr_brightness,
            peak: display.peak.max(config.sdr_brightness),
        }),
        None => BlendSpace::Sdr,
    }
}

fn output_parametric(head: &Head, config: &OutputConfig) -> Parametric {
    let Some(display) = lit_hdr(head, config) else {
        return Parametric::named(NamedPrimaries::Srgb, TransferFunction::Gamma22)
            .expect("gamma 2.2 is a known transfer function");
    };
    let mut parametric = Parametric::named(NamedPrimaries::Bt2020, TransferFunction::St2084Pq)
        .expect("PQ is a known transfer function");
    let description = &mut parametric.description;
    description.luminance.min = display.black;
    description.luminance.max = display.black + PQ_SWING;
    description.luminance.reference = config.sdr_brightness;
    description.mastering = Some(Mastering {
        primaries: display.primaries,
        min: display.black,
        max: display.peak,
    });
    description.max_cll = Some(display.peak);
    description.max_fall = Some(display.frame_average);
    parametric
}

#[derive(Debug, Default)]
pub(crate) struct AnnouncedColors {
    announced: HashMap<OutputId, Parametric>,
}

impl AnnouncedColors {
    pub(crate) fn announce(
        &mut self,
        output: OutputId,
        (head, config): (&Head, &OutputConfig),
        surfaces: &mut Surfaces,
    ) -> bool {
        let parametric = output_parametric(head, config);
        if self.announced.get(&output) == Some(&parametric) {
            return false;
        }
        let first = self.announced.insert(output, parametric).is_none();
        let description = surfaces.describe(parametric);
        surfaces.set_output_image_description(output, description);
        !first
    }

    pub(crate) fn forget(&mut self, output: OutputId) {
        self.announced.remove(&output);
    }
}
