use rikka_image::Image;
use shell_kit::pictures::{self, Within};

fn picture(width: u32, height: u32) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!("picture-{width}x{height}.png"));
    let pixels = vec![200; (width * height * 4) as usize];
    Image::from_rgba8(width, height, pixels)
        .expect("picture")
        .save(&path)
        .expect("save picture");
    path
}

#[test]
fn develop_shrinks_to_the_bounds() {
    let path = picture(400, 200);
    let cases = [
        (Some((100, 100)), Within::Cover, (200, 100)),
        (Some((100, 100)), Within::Contain, (100, 50)),
        (Some((300, 50)), Within::Cover, (300, 150)),
        (Some((301, 301)), Within::Contain, (301, 150)),
        (Some((1000, 1000)), Within::Cover, (400, 200)),
        (None, Within::Contain, (400, 200)),
    ];
    for (bounds, within, expected) in cases {
        let developed = pictures::develop(&path, bounds, within).expect("developed");
        assert_eq!(
            (developed.width, developed.height),
            expected,
            "{bounds:?} {within:?}"
        );
    }
}
