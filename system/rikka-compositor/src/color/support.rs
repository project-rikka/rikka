use rikka_compositor_surface::{
    AlphaMode, Coefficients, ColorFeature, ColorRange, ColorSupport, NamedPrimaries, Parametric,
    RenderIntent, RepresentationSupport, TransferFunction,
};

pub(crate) fn color_support() -> ColorSupport {
    ColorSupport {
        intents: vec![RenderIntent::Perceptual, RenderIntent::Relative],
        features: vec![
            ColorFeature::Parametric,
            ColorFeature::SetPrimaries,
            ColorFeature::SetTfPower,
            ColorFeature::SetLuminances,
            ColorFeature::SetMasteringDisplayPrimaries,
            ColorFeature::ExtendedTargetVolume,
            ColorFeature::WindowsScrgb,
            ColorFeature::WindowsBt2100,
        ],
        transfer_functions: vec![
            TransferFunction::Srgb,
            TransferFunction::CompoundPower24,
            TransferFunction::Gamma22,
            TransferFunction::Gamma28,
            TransferFunction::Bt1886,
            TransferFunction::ExtLinear,
            TransferFunction::St2084Pq,
            TransferFunction::Hlg,
        ],
        primaries: vec![
            NamedPrimaries::Srgb,
            NamedPrimaries::PalM,
            NamedPrimaries::Pal,
            NamedPrimaries::Ntsc,
            NamedPrimaries::GenericFilm,
            NamedPrimaries::Bt2020,
            NamedPrimaries::Cie1931Xyz,
            NamedPrimaries::DciP3,
            NamedPrimaries::DisplayP3,
            NamedPrimaries::AdobeRgb,
        ],
        preferred: Parametric::named(NamedPrimaries::Srgb, TransferFunction::Gamma22)
            .expect("gamma 2.2 is a known transfer function"),
    }
}

pub(crate) fn representation_support() -> RepresentationSupport {
    let ycbcr = [
        Coefficients::Bt601,
        Coefficients::Bt709,
        Coefficients::Bt2020,
    ];
    RepresentationSupport {
        alpha_modes: vec![
            AlphaMode::PremultipliedElectrical,
            AlphaMode::PremultipliedOptical,
            AlphaMode::Straight,
        ],
        coefficients: std::iter::once((Coefficients::Identity, ColorRange::Full))
            .chain(ycbcr.into_iter().flat_map(|coefficients| {
                [
                    (coefficients, ColorRange::Full),
                    (coefficients, ColorRange::Limited),
                ]
            }))
            .collect(),
    }
}
