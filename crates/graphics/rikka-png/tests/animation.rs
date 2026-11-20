use rikka_png::{Blend, Decoder, Dispose, Error, Limits};

use crate::support::{rgba_png, texture};

fn animated() -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    let first = texture(8 * 6 * 4, 1);
    let second = texture(3 * 2 * 4, 2);
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, 8, 6);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_animated(2, 5).expect("two frames");
        let mut writer = encoder.write_header().expect("writes header");
        writer.set_frame_delay(3, 50).expect("sets delay");
        writer.write_image_data(&first).expect("writes frame");
        writer.set_frame_dimension(3, 2).expect("sets size");
        writer.set_frame_position(4, 1).expect("sets position");
        writer.set_frame_delay(1, 0).expect("sets delay");
        writer
            .set_dispose_op(png::DisposeOp::Previous)
            .expect("sets dispose");
        writer.set_blend_op(png::BlendOp::Over).expect("sets blend");
        writer.write_image_data(&second).expect("writes frame");
        writer.finish().expect("finishes");
    }
    (bytes, first, second)
}

#[test]
fn animation_frames_keep_their_placement_timing_and_pixels() {
    let (png, first, second) = animated();

    let animation = Decoder::new()
        .animation(&png, &Limits::UNLIMITED)
        .expect("decodes")
        .expect("is animated");

    assert_eq!((animation.width, animation.height), (8, 6));
    assert_eq!(animation.plays, 5);
    assert_eq!(animation.frames.len(), 2);
    let [opening, patch] = &animation.frames[..] else {
        panic!("two frames");
    };
    assert_eq!(opening.rgba, first);
    assert_eq!(
        (opening.control.left, opening.control.top),
        (0, 0),
        "{:?}",
        opening.control
    );
    assert_eq!((opening.control.width, opening.control.height), (8, 6));
    assert_eq!(
        (
            opening.control.delay_numerator,
            opening.control.delay_denominator
        ),
        (3, 50)
    );
    assert_eq!(opening.control.dispose, Dispose::Keep);
    assert_eq!(opening.control.blend, Blend::Source);
    assert_eq!(patch.rgba, second);
    assert_eq!((patch.control.left, patch.control.top), (4, 1));
    assert_eq!((patch.control.width, patch.control.height), (3, 2));
    assert_eq!(
        (
            patch.control.delay_numerator,
            patch.control.delay_denominator
        ),
        (1, 0)
    );
    assert_eq!(patch.control.dispose, Dispose::Previous);
    assert_eq!(patch.control.blend, Blend::Over);

    let still = Decoder::new()
        .decode(&png, &Limits::UNLIMITED)
        .expect("decodes");
    assert_eq!(still.picture.samples.as_bytes(), first);
}

#[test]
fn stills_are_not_animations_and_frames_count_against_the_limit() {
    let still = rgba_png(4, 4, &texture(64, 3));
    assert_eq!(
        Decoder::new()
            .animation(&still, &Limits::UNLIMITED)
            .expect("parses"),
        None
    );

    let (png, ..) = animated();
    let tight = Limits {
        max_width: 8,
        max_height: 6,
        max_bytes: 8 * 6 * 4 + 3 * 2 * 4 - 1,
    };
    assert!(matches!(
        Decoder::new().animation(&png, &tight),
        Err(Error::LimitsExceeded { .. })
    ));
}
