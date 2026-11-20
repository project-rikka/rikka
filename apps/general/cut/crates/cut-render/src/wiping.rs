use cut_project::{Direction, PresetTransform, Wipe};
use rikka_types::{Point, Rect};

pub(crate) fn wipes(transform: &PresetTransform, text: bool) -> Vec<Wipe> {
    let mut wipes: Vec<Wipe> = transform.wipe.into_iter().collect();
    if !text && transform.reveal < 1.0 {
        wipes.push(Wipe {
            direction: Direction::Left,
            progress: transform.reveal,
        });
    }
    wipes
}

pub(crate) fn clip_to_wipes(wipes: &[Wipe], polygon: Vec<Point>, across: Rect) -> Vec<Point> {
    wipes
        .iter()
        .fold(polygon, |polygon, wipe| match polygon.len() < 3 {
            true => polygon,
            false => wipe.clip_to(&polygon, across),
        })
}
