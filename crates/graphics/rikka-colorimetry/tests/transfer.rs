use rikka_colorimetry::{PQ_PEAK, TransferFunction};

use crate::close;

#[test]
fn transfer_functions_match_their_specifications() {
    let encodings = [
        (TransferFunction::Pq, 100.0 / PQ_PEAK, 0.508_078),
        (TransferFunction::Pq, 203.0 / PQ_PEAK, 0.580_689),
        (TransferFunction::Pq, 1000.0 / PQ_PEAK, 0.751_827),
        (TransferFunction::Pq, 1.0, 1.0),
        (TransferFunction::Hlg, 1.0 / 12.0, 0.5),
        (TransferFunction::Hlg, 0.264_963, 0.75),
        (TransferFunction::Hlg, 1.0, 1.0),
        (TransferFunction::CompoundPower24, 0.214_041, 0.5),
        (TransferFunction::CompoundPower24, 0.003_130_8, 0.040_45),
        (TransferFunction::GAMMA22, 0.217_638, 0.5),
        (TransferFunction::BT1886, 0.5_f64.powf(2.4), 0.5),
        (TransferFunction::Linear, 3.5, 3.5),
    ];
    for (transfer, linear, encoded) in encodings {
        let forward = transfer.encode(linear);
        assert!(
            close(forward, encoded, 1.0e-5),
            "{transfer:?} encodes {linear} as {forward}, not {encoded}"
        );
        let back = transfer.decode(encoded);
        assert!(
            close(back, linear, 1.0e-5),
            "{transfer:?} decodes {encoded} as {back}, not {linear}"
        );
    }
}

#[test]
fn relative_curves_extend_symmetrically_below_zero() {
    for transfer in [
        TransferFunction::GAMMA22,
        TransferFunction::CompoundPower24,
        TransferFunction::Linear,
    ] {
        assert!(close(transfer.decode(-0.5), -transfer.decode(0.5), 1.0e-12));
    }
}
