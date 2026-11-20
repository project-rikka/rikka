use crate::support::{at, imaging, picture, rgba};
use rikka_image::{ColorType, Image};
use rikka_image_edit::{ConvertProfile, Depth, Histogram, HistogramChannel, Profile, Recipe};
use rikka_types::Color;

#[test]
fn eight_bit_pixels_round_trip() {
    let original = picture(37, 23, |x, y| {
        [(x * 7) as u8, (y * 11) as u8, 200, ((x + y) * 5) as u8]
    });
    let image = original.to_image().unwrap();
    assert_eq!(image.color(), ColorType::Rgba8);
    let again = imaging().picture(&image).unwrap();
    assert_eq!(rgba(&again), rgba(&original));
}

#[test]
fn tile_seams_are_invisible() {
    let wide = picture(4100, 3, |x, y| {
        [(x % 251) as u8, y as u8, (x / 256) as u8, 255]
    });
    let back = rgba(&wide);
    for x in [0u32, 4094, 4095, 4096, 4097, 4099] {
        let index = ((2 * 4100 + x) * 4) as usize;
        assert_eq!(
            &back[index..index + 4],
            &[(x % 251) as u8, 2, (x / 256) as u8, 255]
        );
    }
}

#[test]
fn high_depth_pictures_keep_depth() {
    let deep = Image::filled(5, 4, ColorType::Rgb16, Color::rgb(0.25, 0.5, 0.75)).unwrap();
    let picture = imaging().picture(&deep).unwrap();
    assert_eq!(picture.depth(), Depth::Sixteen);
    let back = picture.to_image().unwrap();
    assert_eq!(
        back.color(),
        ColorType::Rgb16,
        "an opaque picture leaves its alpha out"
    );
    assert_eq!(back.as_bytes(), deep.convert(ColorType::Rgb16).as_bytes());

    let float = Image::filled(3, 3, ColorType::Rgba32F, Color::rgba(0.75, 0.25, 0.5, 0.5)).unwrap();
    let picture = imaging().picture(&float).unwrap();
    assert_eq!(picture.depth(), Depth::Float);
    assert_eq!(picture.to_image().unwrap().as_bytes(), float.as_bytes());
}

#[test]
fn icc_profile_round_trips() {
    let tagged = Image::filled(4, 4, ColorType::Rgba8, Color::rgb(0.2, 0.4, 0.6))
        .unwrap()
        .with_icc_profile(Some(Profile::display_p3().icc()));
    let picture = imaging().picture(&tagged).unwrap();
    assert_eq!(
        picture.profile().and_then(|profile| profile.name()),
        Some("Display P3")
    );
    let saved = picture.to_image().unwrap();
    assert_eq!(saved.icc_profile(), Some(&Profile::display_p3().icc()[..]));
}

#[test]
fn profile_conversion_preserves_color() {
    let red = picture(2, 2, |_, _| [255, 0, 0, 255]);
    let wide = red
        .develop(&Recipe::new().then(ConvertProfile::new(Profile::display_p3())))
        .unwrap();
    let [r, g, b, a] = at(&wide, 0, 0);
    assert!(
        r < 245 && g > 20 && b < 40 && a == 255,
        "{:?}",
        [r, g, b, a]
    );
    let back = wide
        .develop(&Recipe::new().then(ConvertProfile::new(Profile::srgb())))
        .unwrap();
    let [r, g, b, _] = at(&back, 0, 0);
    assert!(r >= 253 && g <= 2 && b <= 2, "{:?}", [r, g, b]);
}

#[test]
fn shown_frames_are_premultiplied_srgb() {
    let half = picture(10, 6, |_, _| [200, 100, 50, 128]);
    let shown = half.shown().unwrap();
    assert_eq!((shown.width(), shown.height()), (10, 6));
    assert_eq!(shown.parts().len(), 1);
    assert_eq!(shown.parts()[0].0, [0, 0]);
    let part = half.shown_area([2, 2], [3, 2]).unwrap();
    assert_eq!(part.parts()[0].1.extent(), [3, 2]);
    assert!(half.shown_area([8, 0], [4, 1]).is_err());
}

#[test]
fn histogram_counts_visible_pixels() {
    let split = picture(8, 2, |x, _| match x < 4 {
        true => [0, 0, 0, 255],
        false => [255, 255, 255, 255],
    });
    let histogram: Histogram = split.histogram().unwrap();
    assert_eq!(histogram.total(), 16);
    assert_eq!(histogram.channel(HistogramChannel::Red)[0], 8);
    assert_eq!(histogram.channel(HistogramChannel::Red)[1023], 8);
    assert!((histogram.mean(HistogramChannel::Luminosity) - 0.5).abs() < 0.01);
}

#[test]
fn piece_and_patch_round_trip() {
    let original = picture(4103, 9, |x, y| {
        [
            (x % 253) as u8,
            (y * 17) as u8,
            (x / 256) as u8,
            255 - y as u8,
        ]
    });
    let touched = picture(4103, 9, |x, y| {
        match (4090..4100).contains(&x) && (2..5).contains(&y) {
            true => [1, 2, 3, 4],
            false => [
                (x % 253) as u8,
                (y * 17) as u8,
                (x / 256) as u8,
                255 - y as u8,
            ],
        }
    });

    assert_eq!(original.changed_area(&original).unwrap(), None);
    let area = touched
        .changed_area(&original)
        .unwrap()
        .expect("changed area present");
    assert_eq!(area, [4090, 2, 10, 3]);

    let piece = original.piece(area).unwrap();
    let snapshot = rgba(&touched);
    let restored = touched.patched(&piece, [area[0], area[1]]).unwrap();
    assert!(rgba(&restored) == rgba(&original));
    assert!(
        rgba(&touched) == snapshot,
        "patching leaves the patched picture as it was"
    );
    assert!(touched.patched(&piece, [4100, 0]).is_err());
}
