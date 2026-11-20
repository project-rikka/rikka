use std::time::{Duration, Instant};

use rikka_renderer::{Node, Scene};
use rikka_test_renderer::TestRenderer;
use rikka_types::Rect;
use rikka_video::{EncoderBackend, EncoderConfig, Picture, PixelLayout, VideoEncoder};
use rikka_video_play::{PlaybackState, VideoPlayer};

const WIDTH: u32 = 320;
const HEIGHT: u32 = 240;
const FRAME_TICK: Duration = Duration::from_millis(2);

fn headless_renderer() -> Option<TestRenderer> {
    match rikka_test_renderer::video_renderer("rikka-video-test") {
        Ok(renderer) => Some(renderer),
        Err(err) => {
            eprintln!("skipping: no usable GPU ({err})");
            None
        }
    }
}

fn test_frame(index: u32) -> Vec<u8> {
    let mut pixels = Vec::with_capacity((WIDTH * HEIGHT * 4) as usize);
    for _y in 0..HEIGHT {
        for x in 0..WIDTH {
            pixels.extend_from_slice(&[
                (index * 8).min(255) as u8,
                (x * 255 / WIDTH) as u8,
                200,
                255,
            ]);
        }
    }
    pixels
}

#[test]
fn encode_then_decode_roundtrip() {
    let Some(mut renderer) = headless_renderer() else {
        return;
    };

    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("roundtrip.mp4");

    let config = EncoderConfig::new(WIDTH, HEIGHT).frame_rate(30);
    let mut encoder =
        VideoEncoder::new(Some(renderer.gpu()), &path, config).expect("encoder creation");
    println!("encoder backend: {:?}", encoder.backend());
    for index in 0..30 {
        encoder.push_frame(&test_frame(index)).expect("push frame");
    }
    let stats = encoder.finish().expect("finish");
    assert_eq!(stats.frames, 30);
    assert!(path.exists(), "output file missing");

    let mut player = VideoPlayer::open(&renderer.uploads(), &path).expect("open");
    assert_eq!(player.info().width, WIDTH);
    assert_eq!(player.info().height, HEIGHT);
    println!(
        "decoding hardware-accelerated: {}",
        player.is_hardware_accelerated()
    );
    player.play();

    let mut scene = Scene::new();
    scene.set_background(rikka_types::Color::BLACK);
    scene.insert(Node::image(
        Rect::new(0.0, 0.0, WIDTH as f32, HEIGHT as f32),
        player.texture(),
    ));
    let mut target = renderer
        .create_target(WIDTH, HEIGHT)
        .expect("offscreen target");

    let deadline = Instant::now() + Duration::from_secs(15);
    let mut presented = false;
    while Instant::now() < deadline {
        if player.advance(&renderer.uploads()).expect("advance") {
            scene.notify_texture_updated(player.texture());
            presented = true;
        }
        renderer.draw(&mut scene, &mut target).expect("render");
        if presented {
            break;
        }
        std::thread::sleep(FRAME_TICK);
    }
    assert!(presented, "no frame was presented within the deadline");

    let pixels = renderer.read_pixels(&mut target).expect("read back");
    let center = ((HEIGHT / 2 * WIDTH + WIDTH / 2) * 4) as usize;
    let (r, g, b) = (pixels[center], pixels[center + 1], pixels[center + 2]);
    println!("center pixel: ({r}, {g}, {b})");
    assert!(
        g > 90 && g < 165,
        "green ramp at center should be near 127, got {g}"
    );
    assert!(b > 150, "blue should be near 200, got {b}");

    let deadline = Instant::now() + Duration::from_millis(500);
    let mut frames = 0;
    while Instant::now() < deadline {
        if player.advance(&renderer.uploads()).expect("advance") {
            scene.notify_texture_updated(player.texture());
            renderer.draw(&mut scene, &mut target).expect("render");
            frames += 1;
        }
        std::thread::sleep(FRAME_TICK);
    }
    println!("steady-state frames presented: {frames}");
    assert!(frames > 0, "playback stalled after the first frame");

    player.close(&renderer.uploads());
    rikka_video_play::wait_for_closed_players();
}

#[test]
fn seek_and_loop() {
    let Some(mut renderer) = headless_renderer() else {
        return;
    };
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("seek.mp4");

    let mut encoder = VideoEncoder::new(
        Some(renderer.gpu()),
        &path,
        EncoderConfig::new(WIDTH, HEIGHT).frame_rate(30),
    )
    .expect("encoder creation");
    for index in 0..60 {
        encoder.push_frame(&test_frame(index)).expect("push frame");
    }
    encoder.finish().expect("finish");

    let mut player = VideoPlayer::open(&renderer.uploads(), &path).expect("open");
    player.set_looping(true);
    player.play();
    player.seek(Duration::from_secs(1));

    let mut scene = Scene::new();
    scene.insert(Node::image(
        Rect::new(0.0, 0.0, WIDTH as f32, HEIGHT as f32),
        player.texture(),
    ));
    let mut target = renderer
        .create_target(WIDTH, HEIGHT)
        .expect("offscreen target");

    let deadline = Instant::now() + Duration::from_secs(15);
    let mut presented = 0;
    while Instant::now() < deadline && presented < 10 {
        if player.advance(&renderer.uploads()).expect("advance") {
            scene.notify_texture_updated(player.texture());
            presented += 1;
        }
        renderer.draw(&mut scene, &mut target).expect("render");
        std::thread::sleep(FRAME_TICK);
    }
    assert!(presented >= 10, "looping playback after seek stalled");

    player.close(&renderer.uploads());
    rikka_video_play::wait_for_closed_players();
}

#[test]
fn first_frame_decodes_without_renderer() {
    rikka_test_renderer::init_tracing();

    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("first_frame.mp4");
    let config = EncoderConfig::new(WIDTH, HEIGHT)
        .frame_rate(30)
        .backend(EncoderBackend::Software);
    let mut encoder = match VideoEncoder::new(None, &path, config) {
        Ok(encoder) => encoder,
        Err(err) => {
            eprintln!("skipping: no software encoder ({err})");
            return;
        }
    };
    for index in 0..10 {
        encoder.push_frame(&test_frame(index)).expect("push frame");
    }
    encoder.finish().expect("finish");

    let picture = rikka_video::first_frame(&path).expect("first frame");
    assert_eq!(picture.dimensions(), (WIDTH, HEIGHT));

    let probed = rikka_video::probe(&path).expect("the recording is measured without decoding");
    assert_eq!((probed.width, probed.height), (WIDTH, HEIGHT));

    let center = ((HEIGHT / 2 * WIDTH + WIDTH / 2) * 4) as usize;
    let rgba = picture.rgba8();
    assert_eq!(rgba.len(), (WIDTH * HEIGHT * 4) as usize);
    let (r, g, b) = (rgba[center], rgba[center + 1], rgba[center + 2]);
    println!("center pixel: ({r}, {g}, {b})");
    assert!(
        g > 90 && g < 165,
        "green ramp at center should be near 127, got {g}"
    );
    assert!(b > 150, "blue should be near 200, got {b}");
    assert!(r < 60, "the first frame has no red in it, got {r}");
}

#[test]
fn paused_player_shows_current_frame() {
    let Some(mut renderer) = headless_renderer() else {
        return;
    };
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("paused.mp4");

    let mut encoder = VideoEncoder::new(
        Some(renderer.gpu()),
        &path,
        EncoderConfig::new(WIDTH, HEIGHT).frame_rate(30),
    )
    .expect("encoder creation");
    for index in 0..30 {
        encoder.push_frame(&test_frame(index)).expect("push frame");
    }
    encoder.finish().expect("finish");

    let mut player = VideoPlayer::open(&renderer.uploads(), &path).expect("open");
    let mut scene = Scene::new();
    scene.insert(Node::image(
        Rect::new(0.0, 0.0, WIDTH as f32, HEIGHT as f32),
        player.texture(),
    ));
    let mut target = renderer
        .create_target(WIDTH, HEIGHT)
        .expect("offscreen target");

    let mut shown = |player: &mut VideoPlayer, renderer: &mut TestRenderer| -> u8 {
        let deadline = Instant::now() + Duration::from_secs(15);
        let mut presented = false;
        while Instant::now() < deadline && !presented {
            presented = player.advance(&renderer.uploads()).expect("advance");
            std::thread::sleep(FRAME_TICK);
        }
        assert!(presented, "the paused player never showed a frame");
        scene.notify_texture_updated(player.texture());
        renderer.draw(&mut scene, &mut target).expect("render");
        let pixels = renderer.read_pixels(&mut target).expect("read back");
        pixels[((HEIGHT / 2 * WIDTH + WIDTH / 2) * 4) as usize]
    };

    let first = shown(&mut player, &mut renderer);
    assert_eq!(player.state(), PlaybackState::Paused);
    assert!(
        first < 60,
        "the recording opens on its first frame: {first}"
    );

    player.seek(Duration::from_millis(600));
    let sought = shown(&mut player, &mut renderer);
    assert_eq!(player.state(), PlaybackState::Paused);
    assert!(
        sought.abs_diff(18 * 8) < 40,
        "the paused player moved to the eighteenth frame: {sought}"
    );
    assert!(
        player.position().abs_diff(Duration::from_millis(600)) < Duration::from_millis(120),
        "the position is the frame that is shown: {:?}",
        player.position()
    );

    player.close(&renderer.uploads());
    rikka_video_play::wait_for_closed_players();
}

#[test]
fn frame_placed_by_capture_time() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("uneven.mp4");

    let config = EncoderConfig::new(WIDTH, HEIGHT)
        .frame_rate(30)
        .backend(EncoderBackend::Software);
    let Ok(mut encoder) = VideoEncoder::new(None, &path, config) else {
        eprintln!("skipping: no software encoder in this FFmpeg");
        return;
    };

    let pixels = test_frame(1);
    let picture = || Picture {
        bytes: &pixels,
        width: WIDTH,
        height: HEIGHT,
        stride: WIDTH as usize * 4,
        layout: PixelLayout::Rgba,
    };

    for (index, at) in [0, 500, 1000, 1500].into_iter().enumerate() {
        assert!(
            encoder
                .push_frame_at(picture(), Duration::from_millis(at))
                .expect("a picture encodes"),
            "the frame at {at}ms is the {index}th and has a place of its own"
        );
    }
    assert_eq!(
        encoder.frames(),
        4,
        "a stalled source encodes what it had, not one frame a tick"
    );
    assert!(
        !encoder
            .push_frame_at(picture(), Duration::from_millis(1500))
            .expect("a picture whose place is taken is refused rather than failing"),
        "the same moment is not encoded twice"
    );

    let stats = encoder.finish().expect("finish");
    assert_eq!(stats.frames, 4, "four pictures over a second and a half");
    assert!(
        stats.duration >= Duration::from_millis(1500)
            && stats.duration <= Duration::from_millis(1600),
        "the recording lasts as long as it really did: {:?}",
        stats.duration
    );
}
