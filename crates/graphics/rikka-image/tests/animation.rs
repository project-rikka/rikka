use std::time::Duration;

use gif::{ColorOutput, DecodeOptions, DisposalMethod, Encoder, Repeat};
use rikka_image::{ColorType, Error, Format, Frame, GifWriter, Loops, read};
use rikka_types::Color;

use crate::support::{close, gradient, solid};

const RED: [u8; 4] = [255, 0, 0, 255];
const GREEN: [u8; 4] = [0, 255, 0, 255];
const BLUE: [u8; 4] = [0, 0, 255, 255];
const WHITE: [u8; 4] = [255, 255, 255, 255];
const CLEAR: [u8; 4] = [0, 0, 0, 0];
const PALETTE: [u8; 12] = [255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255];

fn animation(colors: &[Color], delay_ms: u16) -> Vec<u8> {
    let mut bytes = Vec::new();
    {
        let mut encoder = Encoder::new(&mut bytes, 8, 8, &[]).expect("starts");
        encoder.set_repeat(Repeat::Infinite).expect("set repeat");
        for &color in colors {
            let mut pixels = solid(8, 8, color, ColorType::Rgba8).into_rgba8();
            let mut frame = gif::Frame::from_rgba_speed(8, 8, &mut pixels, 10);
            frame.delay = delay_ms / 10;
            encoder.write_frame(&frame).expect("encode frame");
        }
    }
    bytes
}

struct Piece {
    left: u16,
    top: u16,
    width: u16,
    height: u16,
    indices: Vec<u8>,
    transparent: Option<u8>,
    dispose: DisposalMethod,
}

fn pieces(canvas: (u16, u16), pieces: Vec<Piece>) -> Vec<u8> {
    let mut bytes = Vec::new();
    {
        let mut encoder = Encoder::new(&mut bytes, canvas.0, canvas.1, &PALETTE).expect("starts");
        for piece in pieces {
            let mut frame = gif::Frame::from_indexed_pixels(
                piece.width,
                piece.height,
                piece.indices,
                piece.transparent,
            );
            frame.left = piece.left;
            frame.top = piece.top;
            frame.dispose = piece.dispose;
            frame.delay = 5;
            encoder.write_frame(&frame).expect("encode frame");
        }
    }
    bytes
}

fn pixels(frame: &Frame) -> Vec<[u8; 4]> {
    frame.image().rgba8().as_chunks::<4>().0.to_vec()
}

#[test]
fn decodes_every_animation_frame() {
    let red = Color::rgb(1.0, 0.0, 0.0);
    let blue = Color::rgb(0.0, 0.0, 1.0);
    let bytes = animation(&[red, blue], 80);

    let animation = read(&bytes).animation().expect("decodes");

    assert_eq!(animation.frame_count(), 2);
    assert!(animation.is_animated());
    assert_eq!(animation.dimensions(), (8, 8));
    assert_eq!(animation.loops(), Loops::Infinite);
    assert_eq!(animation.frames()[0].delay(), Duration::from_millis(80));
    assert!(close(
        animation.frames()[0].image().pixel(0, 0).expect("inside"),
        red
    ));
    assert!(close(
        animation.frames()[1].image().pixel(0, 0).expect("inside"),
        blue
    ));
    assert_eq!(
        read(&bytes).decode().expect("decodes"),
        *animation.frames()[0].image()
    );
}

#[test]
fn frames_are_placed_keyed_and_disposed_on_the_canvas() {
    let bytes = pieces(
        (4, 4),
        vec![
            Piece {
                left: 0,
                top: 0,
                width: 4,
                height: 4,
                indices: vec![0; 16],
                transparent: None,
                dispose: DisposalMethod::Keep,
            },
            Piece {
                left: 1,
                top: 1,
                width: 2,
                height: 2,
                indices: vec![2, 2, 2, 3],
                transparent: Some(3),
                dispose: DisposalMethod::Background,
            },
            Piece {
                left: 0,
                top: 0,
                width: 1,
                height: 1,
                indices: vec![1],
                transparent: None,
                dispose: DisposalMethod::Previous,
            },
            Piece {
                left: 3,
                top: 3,
                width: 4,
                height: 4,
                indices: vec![3; 16],
                transparent: None,
                dispose: DisposalMethod::Keep,
            },
        ],
    );

    let animation = read(&bytes).animation().expect("decodes");

    assert_eq!(animation.loops(), Loops::Finite(1));
    let frames: Vec<Vec<[u8; 4]>> = animation.frames().iter().map(pixels).collect();
    assert_eq!(frames[0], vec![RED; 16]);
    let mut patched = vec![RED; 16];
    patched[5] = BLUE;
    patched[6] = BLUE;
    patched[9] = BLUE;
    assert_eq!(frames[1], patched);
    let mut cleared = vec![RED; 16];
    for at in [5, 6, 9, 10] {
        cleared[at] = CLEAR;
    }
    let mut marked = cleared.clone();
    marked[0] = GREEN;
    assert_eq!(frames[2], marked);
    let mut clipped = cleared;
    clipped[15] = WHITE;
    assert_eq!(frames[3], clipped);
}

#[test]
fn interlaced_and_local_palette_frames_match_the_oracle() {
    let mut bytes = Vec::new();
    {
        let mut encoder = Encoder::new(&mut bytes, 7, 9, &[]).expect("starts");
        let indices: Vec<u8> = (0..63u8).map(|index| index % 5).collect();
        let mut frame = gif::Frame::from_indexed_pixels(7, 9, indices, None);
        frame.interlaced = true;
        frame.palette = Some((0..15u8).map(|value| value * 17).collect());
        encoder.write_frame(&frame).expect("encode frame");
    }
    let mut options = DecodeOptions::new();
    options.set_color_output(ColorOutput::RGBA);
    let mut oracle = options.read_info(&bytes[..]).expect("oracle reads");
    let expected = oracle
        .read_next_frame()
        .expect("oracle decodes")
        .expect("one frame")
        .buffer
        .to_vec();

    let decoded = read(&bytes).decode().expect("decodes");

    assert_eq!(decoded.dimensions(), (7, 9));
    assert_eq!(decoded.as_bytes(), expected);
}

#[test]
fn zero_delay_frame_uses_default_delay() {
    let bytes = animation(&[Color::WHITE, Color::BLACK], 0);

    let animation = read(&bytes).animation().expect("decodes");

    for frame in animation.frames() {
        assert_eq!(frame.delay(), Duration::from_millis(100));
    }
}

#[test]
fn still_image_decodes_as_single_frame() {
    let source = gradient(8, 8);

    let png = read(source.encode(Format::Png).expect("encodes"))
        .animation()
        .expect("decodes");
    assert_eq!(png.frame_count(), 1);
    assert!(!png.is_animated());
    assert_eq!(png.loops(), Loops::Finite(1));
    assert_eq!(png.still(), &source);

    let webp = read(source.encode(Format::WebP).expect("encodes"))
        .animation()
        .expect("decodes");
    assert_eq!(webp.frame_count(), 1);
    assert!(!webp.is_animated());
}

#[test]
fn animation_frames_count_against_alloc_limit() {
    let bytes = animation(&[Color::WHITE, Color::BLACK, Color::WHITE], 40);

    let error = read(&bytes)
        .max_alloc(600)
        .animation()
        .expect_err("frames exceed alloc limit");

    assert!(matches!(error, Error::LimitsExceeded(_)), "{error:?}");
}

#[test]
fn written_gif_keeps_delays_loops_and_colors_for_any_reader() {
    let red = Color::rgb(1.0, 0.0, 0.0);
    let blue = Color::rgb(0.0, 0.0, 1.0);
    let folder = tempfile::tempdir().expect("create temp dir");
    let path = folder.path().join("clip.gif");

    let mut writer = GifWriter::create(&path, 8, 8, Loops::Finite(3)).expect("creates");
    for (color, delay) in [(red, 120), (blue, 40)] {
        let frame = Frame::new(
            solid(8, 8, color, ColorType::Rgb8),
            Duration::from_millis(delay),
        );
        writer.push(frame).expect("write frame");
    }
    let wrong = Frame::new(solid(4, 4, red, ColorType::Rgb8), Duration::ZERO);
    assert!(matches!(writer.push(wrong), Err(Error::InvalidSize { .. })));
    writer.finish().expect("finishes");
    let bytes = std::fs::read(&path).expect("written file reads");

    let animation = read(&bytes).animation().expect("decodes");
    assert_eq!(animation.frame_count(), 2);
    assert_eq!(animation.loops(), Loops::Finite(3));
    assert_eq!(animation.frames()[0].delay(), Duration::from_millis(120));
    assert_eq!(animation.frames()[1].delay(), Duration::from_millis(40));
    assert!(close(
        animation.frames()[1].image().pixel(4, 4).expect("inside"),
        blue
    ));

    let mut options = DecodeOptions::new();
    options.set_color_output(ColorOutput::RGBA);
    let mut oracle = options.read_info(&bytes[..]).expect("oracle reads");
    assert_eq!((oracle.width(), oracle.height()), (8, 8));
    let first = oracle
        .read_next_frame()
        .expect("oracle decodes")
        .expect("first frame");
    assert_eq!(first.delay, 12);
    assert_eq!(first.buffer.as_chunks::<4>().0, [RED; 64]);
    let second = oracle
        .read_next_frame()
        .expect("oracle decodes")
        .expect("second frame");
    assert_eq!(second.buffer.as_chunks::<4>().0, [BLUE; 64]);
    assert_eq!(oracle.repeat(), Repeat::Finite(3));
}

#[test]
fn photographic_frames_survive_the_palette_closely() {
    let source = gradient(96, 64).convert(ColorType::Rgb8);

    let bytes = source.encode(Format::Gif).expect("encodes");

    let mut options = DecodeOptions::new();
    options.set_color_output(ColorOutput::RGBA);
    let mut oracle = options.read_info(&bytes[..]).expect("oracle reads");
    let frame = oracle
        .read_next_frame()
        .expect("oracle decodes")
        .expect("one frame");
    let original = source.rgba8();
    let worst = frame
        .buffer
        .iter()
        .zip(original.iter())
        .map(|(&written, &wanted)| written.abs_diff(wanted))
        .max();
    assert!(worst <= Some(24), "worst channel error {worst:?}");
    assert_eq!(
        read(&bytes).decode().expect("decodes").as_bytes(),
        &*frame.buffer
    );
}

#[test]
fn animated_png_blends_frames_over_the_canvas() {
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, 4, 4);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_animated(2, 0).expect("two frames");
        let mut writer = encoder.write_header().expect("writes header");
        writer.set_frame_delay(1, 4).expect("sets delay");
        writer
            .write_image_data(&RED.repeat(16))
            .expect("writes frame");
        writer.set_frame_dimension(2, 2).expect("sets size");
        writer.set_frame_position(1, 1).expect("sets position");
        writer.set_blend_op(png::BlendOp::Over).expect("sets blend");
        writer
            .write_image_data(&[0, 0, 255, 128].repeat(4))
            .expect("writes frame");
        writer.finish().expect("finishes");
    }

    let animation = read(&bytes).animation().expect("decodes");

    assert_eq!(animation.frame_count(), 2);
    assert_eq!(animation.loops(), Loops::Infinite);
    assert_eq!(animation.frames()[0].delay(), Duration::from_millis(250));
    let second = animation.frames()[1].image();
    assert!(close(
        second.pixel(1, 1).expect("inside"),
        Color::rgb(0.5, 0.0, 0.5)
    ));
    assert!(close(
        second.pixel(0, 0).expect("inside"),
        Color::rgb(1.0, 0.0, 0.0)
    ));
}
