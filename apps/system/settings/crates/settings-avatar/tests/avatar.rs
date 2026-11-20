use rikka_image::{ColorType, Image};
use rikka_types::Point;
use settings_avatar::{AVATAR_SIDE, Photo, TRANSPARENT_SIDE};

const ACCOUNTS_SERVICE_LIMIT: usize = 1 << 20;

const RED: [u8; 4] = [255, 0, 0, 255];
const BLUE: [u8; 4] = [0, 0, 255, 255];
const CLEAR: [u8; 4] = [0, 0, 0, 0];

fn halves(left: [u8; 4], right: [u8; 4]) -> Photo {
    let (width, height) = (200, 100);
    let pixels = (0..height)
        .flat_map(|_| (0..width).map(|x| if x < width / 2 { left } else { right }))
        .flatten()
        .collect();
    Photo::from_image(Image::from_rgba8(width, height, pixels).unwrap()).unwrap()
}

fn rgba(image: &Image, x: u32, y: u32) -> [u8; 4] {
    let at = ((y * image.width() + x) * 4) as usize;
    image.rgba8()[at..at + 4].try_into().unwrap()
}

#[test]
fn avatar_is_the_framed_square_at_full_size() {
    let photo = halves(RED, BLUE);
    for (by, inside) in [(-100.0, RED), (100.0, BLUE)] {
        let framing = photo.framing().panned(Point::new(by, 0.0));
        let avatar = Image::from_bytes(&photo.avatar(&framing).unwrap()).unwrap();
        assert_eq!(avatar.dimensions(), (AVATAR_SIDE, AVATAR_SIDE));
        assert_eq!(avatar.color(), ColorType::Rgb8);
        for (x, y) in [
            (0, 0),
            (AVATAR_SIDE / 2, AVATAR_SIDE / 2),
            (AVATAR_SIDE - 1, AVATAR_SIDE - 1),
        ] {
            assert_eq!(rgba(&avatar, x, y), inside, "panned {by} at {x},{y}");
        }
    }
}

#[test]
fn transparent_photos_keep_their_alpha() {
    let photo = halves(CLEAR, BLUE);
    let framing = photo.framing().panned(Point::new(-100.0, 0.0));
    let avatar = Image::from_bytes(&photo.avatar(&framing).unwrap()).unwrap();
    assert_eq!(avatar.dimensions(), (TRANSPARENT_SIDE, TRANSPARENT_SIDE));
    assert_eq!(avatar.color(), ColorType::Rgba8);
    assert_eq!(
        rgba(&avatar, TRANSPARENT_SIDE / 2, TRANSPARENT_SIDE / 2)[3],
        0
    );
}

#[test]
fn incompressible_avatars_fit_accounts_service() {
    let side = 600;
    let mut state = 0x9e37_79b9_u32;
    for alpha in [false, true] {
        let pixels = (0..side * side * 4)
            .map(|index| {
                state ^= state << 13;
                state ^= state >> 17;
                state ^= state << 5;
                match !alpha && index % 4 == 3 {
                    true => u8::MAX,
                    false => state.to_le_bytes()[0],
                }
            })
            .collect();
        let photo = Photo::from_image(Image::from_rgba8(side, side, pixels).unwrap()).unwrap();
        let avatar = photo.avatar(&photo.framing()).unwrap();
        assert!(
            avatar.len() <= ACCOUNTS_SERVICE_LIMIT,
            "alpha {alpha}: {} bytes",
            avatar.len()
        );
    }
}
