use rikka_gpu::Compute;
use rikka_gpu::{ComputedCopy, ComputedFrame, ImageFormat};
use rikka_renderer::{Error, Node, Scene};
use rikka_test_renderer::TestRenderer;
use rikka_types::{Color, Rect};

fn renderer() -> TestRenderer {
    rikka_test_renderer::compute_renderer("rikka-renderer-compute")
        .expect("a Vulkan 1.3 device is required for these tests")
}

fn compute(renderer: &TestRenderer) -> Compute {
    renderer
        .gpu()
        .compute()
        .expect("the test device can run compute work")
}

fn gradient(width: u32, height: u32) -> Vec<u8> {
    (0..height)
        .flat_map(|y| (0..width).map(move |x| (x, y)))
        .flat_map(|(x, y)| [(x * 16) as u8, (y * 16) as u8, 7, 255])
        .collect()
}

#[test]
fn image_upload_download_round_trips() {
    let renderer = renderer();
    let compute = compute(&renderer);
    let image = compute.image(8, 4, ImageFormat::Rgba8).unwrap();
    let pixels = gradient(8, 4);

    let mut batch = compute.batch();
    batch.upload(&image, [0, 0], [8, 4], &pixels);
    let back = batch.download(&image, [0, 0], [8, 4]);
    batch.submit().unwrap();

    assert_eq!(back.wait().unwrap(), pixels);
}

#[test]
fn buffer_write_fill_read_back() {
    let renderer = renderer();
    let compute = compute(&renderer);
    let buffer = compute.buffer(16).unwrap();

    let mut batch = compute.batch();
    batch.fill(&buffer, 0x0101_0101);
    batch.write(&buffer, 4, &[9, 8, 7, 6]);
    let back = batch.download_buffer(&buffer);
    batch.submit().unwrap();

    assert_eq!(
        back.wait().unwrap(),
        [1, 1, 1, 1, 9, 8, 7, 6, 1, 1, 1, 1, 1, 1, 1, 1]
    );
}

#[test]
fn uploads_rows_with_stride() {
    let renderer = renderer();
    let compute = compute(&renderer);
    let image = compute.image(3, 2, ImageFormat::Rgba8).unwrap();
    let wide = gradient(5, 2);

    let mut batch = compute.batch();
    batch.upload_rows(&image, [0, 0], [3, 2], &wide[4..], 5 * 4);
    let back = batch.download(&image, [0, 0], [3, 2]);
    batch.submit().unwrap();

    let back = back.wait().unwrap();
    assert_eq!(&back[..12], &wide[4..16]);
    assert_eq!(&back[12..], &wide[24..36]);
}

#[test]
fn submit_reports_invalid_batch() {
    let renderer = renderer();
    let compute = compute(&renderer);
    let image = compute.image(4, 4, ImageFormat::Rgba8).unwrap();

    let mut batch = compute.batch();
    batch.upload(&image, [2, 2], [4, 4], &[0; 64]);
    assert!(matches!(batch.submit(), Err(Error::Invalid(_))));

    let mut batch = compute.batch();
    let early = batch.download(&image, [0, 0], [1, 1]);
    assert!(matches!(early.wait(), Err(Error::Invalid(_))));
    drop(batch);
}

#[test]
fn computed_image_draws_without_readback() {
    let mut renderer = renderer();
    let compute = compute(&renderer);
    let image = compute.image(16, 16, ImageFormat::Rgba8).unwrap();
    let mut batch = compute.batch();
    batch.clear(&image, [0.0, 0.0, 1.0, 1.0]);
    let ticket = batch.submit().unwrap();

    let texture = renderer.textures().create_compute_target(16, 16).unwrap();
    renderer
        .uploads()
        .copy_computed(
            texture,
            [0, 0],
            ComputedCopy::whole(&image, &ticket).unwrap(),
        )
        .unwrap();

    let mut target = renderer.create_target(16, 16).unwrap();
    let mut scene = Scene::new();
    scene.set_background(Color::rgb(1.0, 0.0, 0.0));
    scene.insert(Node::image(Rect::new(0.0, 0.0, 16.0, 16.0), texture));
    renderer.draw(&mut scene, &mut target).unwrap();
    let pixels = renderer.read_pixels(&mut target).unwrap();
    assert_eq!(&pixels[(8 * 16 + 8) * 4..][..4], &[0, 0, 255, 255]);
}

#[test]
fn computed_frame_notifies_when_ready() {
    let renderer = renderer();
    let compute = compute(&renderer);
    let images = compute
        .images(&[[8, 8], [8, 8]], ImageFormat::Rgba8)
        .unwrap();
    let mut batch = compute.batch();
    for image in &images {
        batch.clear(image, [0.0, 1.0, 0.0, 1.0]);
    }
    let ticket = batch.submit().unwrap();
    let parts = images
        .iter()
        .zip([[0, 0], [8, 0]])
        .map(|(image, at)| (at, ComputedCopy::whole(image, &ticket).unwrap()))
        .collect();
    let frame = ComputedFrame::new(16, 8, parts).unwrap();

    let (ready, notified) = std::sync::mpsc::channel();
    frame.on_ready(move || ready.send(()).unwrap());
    notified
        .recv_timeout(std::time::Duration::from_secs(10))
        .expect("on_ready fires once the batch completes");
    assert!(frame.is_ready());

    let (again, late) = std::sync::mpsc::channel();
    frame.on_ready(move || again.send(()).unwrap());
    late.try_recv()
        .expect("a finished frame notifies immediately");
}
