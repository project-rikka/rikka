use rikka_colorimetry::{Luminance, Primaries, TransferFunction as Function};
use rikka_compositor_protocols::wp_color_manager_v1::{Primaries as Named, TransferFunction};

use super::description::Transfer;

const PQ_SWING: f64 = 10_000.0;

pub(super) fn primaries(named: Named) -> Primaries {
    match named {
        Named::Srgb => Primaries::SRGB,
        Named::PalM => Primaries::PAL_M,
        Named::Pal => Primaries::PAL,
        Named::Ntsc => Primaries::NTSC,
        Named::GenericFilm => Primaries::GENERIC_FILM,
        Named::Bt2020 => Primaries::BT2020,
        Named::Cie1931Xyz => Primaries::CIE1931_XYZ,
        Named::DciP3 => Primaries::DCI_P3,
        Named::DisplayP3 => Primaries::DISPLAY_P3,
        Named::AdobeRgb => Primaries::ADOBE_RGB,
    }
}

pub(super) fn function(transfer: Transfer) -> Option<Function> {
    match transfer {
        Transfer::Power(exponent) => Some(Function::Power(exponent)),
        Transfer::Named(named) => match named {
            TransferFunction::Bt1886 => Some(Function::BT1886),
            TransferFunction::Gamma22 | TransferFunction::Srgb => Some(Function::GAMMA22),
            TransferFunction::Gamma28 => Some(Function::GAMMA28),
            TransferFunction::ExtLinear => Some(Function::Linear),
            TransferFunction::St2084Pq => Some(Function::Pq),
            TransferFunction::Hlg => Some(Function::Hlg),
            TransferFunction::CompoundPower24 => Some(Function::CompoundPower24),
            TransferFunction::St240
            | TransferFunction::Log100
            | TransferFunction::Log316
            | TransferFunction::Xvycc
            | TransferFunction::ExtSrgb
            | TransferFunction::St428 => None,
        },
    }
}

pub(super) fn default_luminance(transfer: Transfer) -> Luminance {
    let (min, max, reference) = match transfer {
        Transfer::Named(TransferFunction::Bt1886) => (0.01, 100.0, 100.0),
        Transfer::Named(TransferFunction::St2084Pq) => (0.005, 0.005 + PQ_SWING, 203.0),
        Transfer::Named(TransferFunction::Hlg) => (0.005, 1000.0, 203.0),
        _ => (0.2, 80.0, 80.0),
    };
    Luminance {
        min,
        max,
        reference,
    }
}

pub(super) fn pq_swing(luminance: &mut Luminance) {
    luminance.max = luminance.min + PQ_SWING;
}
