use rikka_colorimetry::{
    BlendSpace, Description, Eetf, HdrSpace, Intent, Luminance, Mapping, PQ_PEAK, Primaries,
    TransferFunction,
};

use crate::close;

const HDR: BlendSpace = BlendSpace::Hdr(HdrSpace {
    reference: 203.0,
    peak: 1000.0,
});

fn pq(nits: f64) -> f64 {
    TransferFunction::Pq.encode(nits / PQ_PEAK)
}

fn grey(value: f64) -> [f64; 3] {
    [value; 3]
}

fn pq_mastered(peak: f64) -> Description {
    Description {
        max_cll: Some(peak),
        ..Description::BT2100_PQ
    }
}

fn hlg() -> Description {
    Description {
        transfer: TransferFunction::Hlg,
        luminance: Luminance {
            min: 0.005,
            max: 1000.0,
            reference: 203.0,
        },
        ..Description::BT2100_PQ
    }
}

#[test]
fn srgb_on_sdr_is_the_identity() {
    let mapping = Mapping::new(&Description::SRGB, Intent::Perceptual, &BlendSpace::Sdr).unwrap();
    assert!(mapping.is_identity());
    let hdr = Mapping::new(&Description::SRGB, Intent::Perceptual, &HDR).unwrap();
    assert!(!hdr.is_identity());
}

#[test]
fn reference_white_lands_on_the_output_reference() {
    let cases = [
        (Description::SRGB, grey(1.0), HDR, 1.0),
        (Description::SRGB, grey(0.5), HDR, 0.217_638),
        (pq_mastered(1000.0), grey(pq(203.0)), HDR, 1.0),
        (pq_mastered(1000.0), grey(pq(406.0)), HDR, 2.0),
        (hlg(), grey(0.75), HDR, 1.0),
        (pq_mastered(203.0), grey(pq(203.0)), BlendSpace::Sdr, 1.0),
    ];
    for (description, encoded, space, expected) in cases {
        let mapped = Mapping::new(&description, Intent::Perceptual, &space)
            .unwrap()
            .apply(encoded);
        for channel in mapped {
            assert!(
                close(channel, expected, 2.0e-3),
                "{:?} {encoded:?} on {space:?} maps to {mapped:?}, not {expected}",
                description.transfer
            );
        }
    }
}

#[test]
fn srgb_red_stays_inside_bt2020() {
    let red = Mapping::new(&Description::SRGB, Intent::Perceptual, &HDR)
        .unwrap()
        .apply([1.0, 0.0, 0.0]);
    let expected = [0.6274, 0.0691, 0.0164];
    for (channel, expected) in red.iter().zip(expected) {
        assert!(close(*channel, expected, 1.0e-4), "{red:?}");
    }
}

#[test]
fn hdr_peak_reaches_sdr_white_and_shadows_pass_unchanged() {
    let mapping = Mapping::new(&pq_mastered(1000.0), Intent::Perceptual, &BlendSpace::Sdr).unwrap();
    let peak = mapping.apply(grey(pq(1000.0)));
    assert!(close(peak[0], 1.0, 1.0e-6), "{peak:?}");
    let shadow = mapping.apply(grey(pq(20.0)));
    let unmapped = TransferFunction::GAMMA22.encode(20.0 / 203.0);
    assert!(close(shadow[0], unmapped, 1.0e-6), "{shadow:?}");
    let white = mapping.apply(grey(pq(203.0)))[0];
    assert!(white < 1.0 && white > 0.8, "{white}");
}

#[test]
fn relative_intent_clips_instead_of_compressing() {
    let mapping = Mapping::new(&pq_mastered(1000.0), Intent::Relative, &BlendSpace::Sdr).unwrap();
    assert!(mapping.tone.is_none());
    assert!(close(mapping.apply(grey(pq(203.0)))[0], 1.0, 1.0e-6));
    assert!(close(mapping.apply(grey(pq(1000.0)))[0], 1.0, 1.0e-6));
}

#[test]
fn eetf_keeps_its_endpoints_and_order() {
    let eetf = Eetf::new(203.0, 1000.0 / 203.0, 1.0).unwrap();
    let knee_nits = TransferFunction::Pq.decode(eetf.knee * eetf.source_peak) * PQ_PEAK;
    let below = knee_nits / 203.0 * 0.99;
    assert!(close(eetf.map(below), below, 1.0e-12));
    assert!(close(eetf.map(1000.0 / 203.0), 1.0, 1.0e-9));
    let mut previous = 0.0;
    for step in 1..=200 {
        let mapped = eetf.map(f64::from(step) * 1000.0 / 203.0 / 200.0);
        assert!(mapped >= previous, "not monotonic at step {step}");
        assert!(mapped <= 1.0 + 1.0e-9);
        previous = mapped;
    }
    assert!(Eetf::new(203.0, 1.0, 1.0).is_none());
}

#[test]
fn hdr_space_encodes_reference_white_as_pq() {
    let space = HdrSpace {
        reference: 203.0,
        peak: 1000.0,
    };
    let encoded = space.encode([1.0, 0.0, -1.0]);
    assert!(close(encoded[0], pq(203.0), 1.0e-9));
    assert!(close(encoded[1], pq(0.0), 1.0e-9));
    assert!(close(encoded[2], pq(0.0), 1.0e-9));
}

#[test]
fn display_p3_on_sdr_clips_out_of_gamut_green() {
    let display_p3 = Description {
        primaries: Primaries::DISPLAY_P3,
        ..Description::SRGB
    };
    let green = Mapping::new(&display_p3, Intent::Perceptual, &BlendSpace::Sdr)
        .unwrap()
        .apply([0.0, 1.0, 0.0]);
    assert_eq!(green[0], 0.0);
    assert!(close(green[1], 1.0, 1.0e-9));
    assert_eq!(green[2], 0.0);
}
