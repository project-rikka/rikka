use discord_stream::{FrameRate, Quality, Resolution};

#[test]
fn fit_scales_to_height_keeping_aspect() {
    let quality = Quality {
        resolution: Resolution::Hd,
        frame_rate: FrameRate::Thirty,
    };
    assert_eq!(quality.fit((1920, 1080)), (1280, 720));
    assert_eq!(quality.fit((1280, 720)), (1280, 720));
    assert_eq!(quality.fit((801, 601)), (800, 600));
    assert_eq!(quality.fit((3440, 1440)), (1720, 720));
    let source = Quality {
        resolution: Resolution::Source,
        ..quality
    };
    assert_eq!(source.fit((5120, 2880)), (3840, 2160));
    assert_eq!(source.fit((2, 2)), (16, 16));
}
