use crate::common::{Server, State, Wayland, output};
use rikka_colorimetry::{Mastering, Primaries, TransferFunction as Function};
use rikka_compositor_surface::{
    AlphaMode, Changes, Coefficients, ColorFeature, ColorRange, ColorSupport, NamedPrimaries,
    Parametric, RenderIntent, RepresentationSupport, Transfer, TransferFunction,
};
use rikka_wayland_client::wl_shm::Format;
use rikka_wayland_client::{wl_output, wp_color_manager_v1};

fn support() -> ColorSupport {
    ColorSupport {
        intents: vec![RenderIntent::Perceptual, RenderIntent::Relative],
        features: vec![
            ColorFeature::Parametric,
            ColorFeature::SetPrimaries,
            ColorFeature::SetLuminances,
            ColorFeature::SetMasteringDisplayPrimaries,
            ColorFeature::ExtendedTargetVolume,
            ColorFeature::WindowsScrgb,
        ],
        transfer_functions: vec![
            TransferFunction::Srgb,
            TransferFunction::Gamma22,
            TransferFunction::St2084Pq,
            TransferFunction::CompoundPower24,
        ],
        primaries: vec![NamedPrimaries::Srgb, NamedPrimaries::Bt2020],
        preferred: Parametric::named(NamedPrimaries::Srgb, TransferFunction::Gamma22).unwrap(),
    }
}

fn enabled(state: &mut State) {
    crate::common::serve_output(state);
    let display = state.display.clone();
    state.surfaces.enable_color_management(&display, support());
    state
        .surfaces
        .set_representation_support(RepresentationSupport {
            alpha_modes: vec![AlphaMode::PremultipliedElectrical, AlphaMode::Straight],
            coefficients: vec![
                (Coefficients::Identity, ColorRange::Full),
                (Coefficients::Bt709, ColorRange::Limited),
            ],
        });
}

fn hdr_output() -> Parametric {
    let mut parametric =
        Parametric::named(NamedPrimaries::Bt2020, TransferFunction::St2084Pq).unwrap();
    parametric.description.luminance.reference = 250.0;
    parametric.description.mastering = Some(Mastering {
        primaries: Primaries::DISPLAY_P3,
        min: 0.01,
        max: 600.0,
    });
    parametric.description.max_cll = Some(600.0);
    parametric
}

fn manager(client: &Wayland) -> wp_color_manager_v1::WpColorManagerV1 {
    client.global(3)
}

fn pq_bt2020(
    manager: &wp_color_manager_v1::WpColorManagerV1,
    tag: u32,
) -> rikka_wayland_client::wp_image_description_v1::WpImageDescriptionV1 {
    let creator = manager.create_parametric_creator();
    creator.set_tf_named(wp_color_manager_v1::TransferFunction::St2084Pq);
    creator.set_primaries_named(wp_color_manager_v1::Primaries::Bt2020);
    creator.create().with_key(tag)
}

fn received(client: &Wayland, line: &str) -> bool {
    client
        .received
        .color
        .iter()
        .any(|received| received == line)
}

#[test]
fn parametric_description_applies_on_commit() {
    let server = Server::with(enabled);
    let mut client = server.connect();
    let manager = manager(&client);
    let compositor = client.compositor();
    let surface = client.surface(&compositor);
    let description = pq_bt2020(&manager, 1);
    let color = manager.get_surface(&surface);
    client.roundtrip();
    assert!(
        client
            .received
            .color
            .iter()
            .any(|line| line.starts_with("ready 1 "))
    );
    color.set_image_description(&description, wp_color_manager_v1::RenderIntent::Relative);
    description.destroy();
    let shm = client.canvas(8, 8, Format::Argb8888);
    surface.attach(Some(&shm.buffer), 0, 0);
    surface.commit();
    client.roundtrip();
    let id = surface.id();
    let (parametric, intent, changes) = server.call(move |state| {
        let surface = state.surface(id);
        let entry = state.surfaces.get(surface).unwrap();
        let (description, intent) = entry.image_description().unwrap();
        (
            *description.parametric(),
            intent,
            state.commits_of(surface)[0].changes,
        )
    });
    assert_eq!(
        parametric.transfer,
        Transfer::Named(TransferFunction::St2084Pq)
    );
    assert_eq!(parametric.description.transfer, Function::Pq);
    assert_eq!(parametric.named_primaries, Some(NamedPrimaries::Bt2020));
    assert_eq!(parametric.description.primaries, Primaries::BT2020);
    assert!((parametric.description.luminance.max - 10_000.005).abs() < 1e-6);
    assert_eq!(intent, RenderIntent::Relative);
    assert!(changes.contains(Changes::COLOR));

    color.destroy();
    surface.commit();
    client.roundtrip();
    let unset = server.call(move |state| {
        let entry = state.surfaces.get(state.surface(id)).unwrap();
        entry.image_description().is_none()
    });
    assert!(unset);
}

#[test]
fn output_description_reports_its_parameters_and_changes() {
    let server = Server::with(|state| {
        enabled(state);
        let description = state.surfaces.describe(hdr_output());
        state
            .surfaces
            .set_output_image_description(output(7), description);
    });
    let mut client = server.connect();
    let manager = manager(&client);
    let display_output: wl_output::WlOutput = client.global(4);
    let colors = manager.get_output(&display_output);
    let description = colors.get_image_description().with_key(5u32);
    client.roundtrip();
    assert!(
        client
            .received
            .color
            .iter()
            .any(|line| line.starts_with("ready 5 "))
    );
    description.get_information();
    client.roundtrip();
    for line in [
        "info primaries named 6",
        "info tf 11",
        "info luminances 50 10000 250",
        "info target luminance 100 600",
        "info done",
    ] {
        assert!(received(&client, line), "missing {line:?}");
    }

    server.call(|state| {
        let sdr = Parametric::named(NamedPrimaries::Srgb, TransferFunction::Gamma22).unwrap();
        let description = state.surfaces.describe(sdr);
        state
            .surfaces
            .set_output_image_description(output(7), description);
    });
    client.roundtrip();
    assert!(received(&client, "output changed"));
}

#[test]
fn preferred_description_follows_the_compositor() {
    let server = Server::with(enabled);
    let mut client = server.connect();
    let manager = manager(&client);
    let compositor = client.compositor();
    let surface = client.surface(&compositor);
    let feedback = manager.get_surface_feedback(&surface);
    client.roundtrip();
    let id = surface.id();
    let identity = server.call(move |state| {
        let surface = state.surface(id);
        let description = state.surfaces.describe(hdr_output());
        let identity = description.identity();
        state
            .surfaces
            .set_preferred_image_description(surface, description);
        identity
    });
    client.roundtrip();
    assert!(received(&client, &format!("preferred {identity}")));
    let preferred = feedback.get_preferred().with_key(9u32);
    client.roundtrip();
    assert!(received(&client, &format!("ready 9 {identity}")));
    preferred.get_information();
    client.roundtrip();
    assert!(received(&client, "info tf 11"));
    assert!(received(&client, "info luminances 50 10000 250"));
}

#[test]
fn degenerate_primaries_fail_as_unsupported() {
    let server = Server::with(enabled);
    let mut client = server.connect();
    let manager = manager(&client);
    let creator = manager.create_parametric_creator();
    creator.set_tf_named(wp_color_manager_v1::TransferFunction::Gamma22);
    creator.set_primaries(
        200_000, 200_000, 300_000, 300_000, 400_000, 400_000, 312_700, 329_000,
    );
    let _description = creator.create().with_key(3u32);
    client.roundtrip();
    assert!(received(&client, "failed 3 1"));
}

#[test]
fn advertised_transfer_functions_follow_the_bound_version() {
    let server = Server::with(enabled);
    let mut client = server.connect();
    let _modern = manager(&client);
    client.roundtrip();
    assert!(received(&client, "tf 14"));
    assert!(!received(&client, "tf 9"));
    let mut legacy_client = server.connect();
    let _legacy: wp_color_manager_v1::WpColorManagerV1 = legacy_client.global(1);
    legacy_client.roundtrip();
    assert!(received(&legacy_client, "tf 9"));
    assert!(!received(&legacy_client, "tf 14"));
}
