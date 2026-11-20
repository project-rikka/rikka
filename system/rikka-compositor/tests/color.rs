use rikka_colorimetry::{BlendSpace, Description, Intent, Mapping, PQ_PEAK, TransferFunction};
use rikka_compositor::Extent;
use rikka_wayland_client::wp_color_manager_v1::{
    Primaries, RenderIntent, TransferFunction as Named, WpColorManagerV1,
};

use crate::common::{Harness, shows};

const EXTENT: Extent = Extent {
    width: 128,
    height: 96,
};

#[test]
fn pq_window_is_tone_mapped_on_an_sdr_display() {
    let harness = Harness::headless(EXTENT);
    let mut client = harness.client();
    let manager: WpColorManagerV1 = client.global(1);
    let window = client.toplevel(1);
    let surface = window.surface.clone();
    client.ack(&window.xdg, window.tag);

    let creator = manager.create_parametric_creator();
    creator.set_tf_named(Named::St2084Pq);
    creator.set_primaries_named(Primaries::Bt2020);
    let pq = creator.create();
    manager
        .get_surface(&surface)
        .set_image_description(&pq, RenderIntent::Perceptual);

    let signal = (TransferFunction::Pq.encode(203.0 / PQ_PEAK) * 255.0).round() as u8;
    let paint = client.pixels(16, 16, 1);
    paint.fill(0..16, [signal, signal, signal, 255]);
    client.show(&surface, &paint);

    let mut description = Description::BT2100_PQ;
    description.luminance.min = 0.005;
    description.luminance.max = 10_000.005;
    let mapped = Mapping::new(&description, Intent::Perceptual, &BlendSpace::Sdr)
        .unwrap()
        .apply([f64::from(signal) / 255.0; 3]);
    let grey = (mapped[0] * 255.0).round() as u8;
    harness.frame_where(move |snapshot| shows(snapshot, 52, 52, [grey, grey, grey, 255]));
}
