use std::sync::mpsc;
use std::time::Duration;

use animate_booth::{Booth, Order, Performer, Picture, Sitter};
use rikka_types::PixelSize;

const FIGURE: &str = "crates/model/rikka-mmd/assets/mmd/miku.pmx";
const MOTION: &str = "crates/model/rikka-mmd/assets/mmd/motion.vmd";
const PATIENCE: Duration = Duration::from_secs(50);
const DIFFERENT: u32 = 40;
const SHARE_DRAWN: f32 = 0.03;

fn drawn_share(picture: &Picture) -> f32 {
    let backdrop = &picture.rgba[..4];
    let differing = picture
        .rgba
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|pixel| {
            pixel[..3]
                .iter()
                .zip(&backdrop[..3])
                .map(|(one, two)| u32::from(one.abs_diff(*two)))
                .sum::<u32>()
                > DIFFERENT
        })
        .count();
    differing as f32 / (picture.width * picture.height) as f32
}

#[test]
fn booth_renders_portrait_and_mannequin_motion() {
    let booth = Booth::new().expect("a booth");
    let size = PixelSize::new(160, 200);
    let (sender, receiver) = mpsc::channel();
    let sitters = [
        Sitter::Model(FIGURE.into()),
        Sitter::Motion {
            motion: MOTION.into(),
            performer: Performer::Mannequin,
        },
    ];
    let tickets: Vec<_> = sitters
        .into_iter()
        .map(|sitter| {
            let sender = sender.clone();
            let label = format!("{sitter:?}");
            booth.render(Order { sitter, size }, move |result| {
                sender.send((label, result)).expect("the test waits");
            })
        })
        .collect();
    for _ in 0..tickets.len() {
        let (label, result) = receiver.recv_timeout(PATIENCE).expect("a picture in time");
        let picture = result.unwrap_or_else(|error| panic!("{label}: {error}"));
        assert_eq!((picture.width, picture.height), (size.width, size.height));
        assert_eq!(picture.rgba.len(), (size.width * size.height * 4) as usize);
        let share = drawn_share(&picture);
        assert!(
            share > SHARE_DRAWN,
            "{label}: {share} of the picture differs from the backdrop"
        );
    }
}
