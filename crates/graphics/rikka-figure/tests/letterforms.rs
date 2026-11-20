use rikka_figure::{LetterPose, LetterWipe, Letterforms, Lettering};
use rikka_image::ColorType;
use rikka_image_edit::{Depth, Imaging};
use rikka_types::{Color, Point, Rect};

const SIDE: u32 = 240;

fn caption() -> Lettering {
    Lettering {
        text: "H I".into(),
        size: 80.0,
        weight: Lettering::BOLD,
        color: Color::WHITE,
        origin: (40, 60),
        ..Lettering::default()
    }
}

fn drawn(imaging: &Imaging, letters: &Letterforms, poses: &[LetterPose]) -> Vec<[u8; 4]> {
    imaging
        .blank(SIDE, SIDE, Depth::Eight, Color::TRANSPARENT)
        .expect("blank image")
        .develop(&letters.recipe(poses, Point::ZERO))
        .expect("failed to draw letters")
        .to_image()
        .expect("failed to read back image")
        .convert(ColorType::Rgba8)
        .as_bytes()
        .as_chunks::<4>()
        .0
        .to_vec()
}

fn inked(imaging: &Imaging, letters: &Letterforms, poses: &[LetterPose]) -> Vec<bool> {
    drawn(imaging, letters, poses)
        .iter()
        .map(|pixel| pixel[3] > 128)
        .collect()
}

fn within(shown: &[bool], area: Rect) -> usize {
    (0..SIDE * SIDE)
        .filter(|index| {
            let spot = Point::new((index % SIDE) as f32 + 0.5, (index / SIDE) as f32 + 0.5);
            shown[*index as usize] && area.contains(spot)
        })
        .count()
}

#[test]
fn letters_pose_independently() {
    let imaging = Imaging::headless().expect("no Vulkan 1.3 compute device");
    let letters = Letterforms::new(&caption(), 1.0);
    assert_eq!(letters.len(), 2);

    let hidden = LetterPose {
        opacity: 0.0,
        ..LetterPose::REST
    };
    let rest = inked(&imaging, &letters, &[]);
    let still = letters.bounds(&[]).expect("ink");
    let middle = letters
        .bounds(&[LetterPose::REST, hidden])
        .expect("ink")
        .max_x();
    let left = Rect::new(0.0, 0.0, middle, SIDE as f32);
    let right = Rect::new(middle, 0.0, SIDE as f32 - middle, SIDE as f32);
    assert!(within(&rest, left) > 100 && within(&rest, right) > 100);

    let alone = inked(&imaging, &letters, &[LetterPose::REST, hidden]);
    assert_eq!(within(&alone, left), within(&rest, left));
    assert_eq!(within(&alone, right), 0);

    let lowered = LetterPose {
        offset: Point::new(0.0, 60.0),
        ..LetterPose::REST
    };
    let dropped = inked(&imaging, &letters, &[lowered]);
    let below = Rect::new(0.0, still.max_y() + 2.0, middle, SIDE as f32);
    assert!(within(&dropped, below) > 100);
    assert!(within(&rest, below) == 0);
    assert!(letters.bounds(&[lowered]).expect("ink").max_y() > still.max_y() + 50.0);

    let half = LetterPose {
        wipe: Some(LetterWipe {
            from: Point::new(-1.0, 0.0),
            progress: 0.5,
        }),
        ..LetterPose::REST
    };
    let wiped = inked(&imaging, &letters, &[half]);
    assert!(within(&wiped, left) > 20);
    assert!(within(&wiped, left) < within(&rest, left) * 3 / 4);

    let soft = LetterPose {
        blur: 6.0,
        ..LetterPose::REST
    };
    assert!(letters.bounds(&[soft]).expect("ink").min_x() < still.min_x() - 10.0);
    let blurred = inked(&imaging, &letters, &[soft]);
    assert!(blurred != rest);
    assert_eq!(within(&blurred, right), within(&rest, right));
}
