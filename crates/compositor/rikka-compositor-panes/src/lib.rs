mod grow;
mod shape;

use rikka_compositor_geometry::{Logical, Rect, Region, Size};
use rikka_types::RoundedRect;

use grow::grown;
use shape::Step;

const MOST: usize = 16;

const SLACK: i32 = 1;

pub fn read_panes(
    region: &Region<Logical>,
    surface: Size<i32, Logical>,
    mut pane: impl FnMut(RoundedRect),
) {
    let whole = region.intersect(Rect::from_size(surface));
    let mut open = whole.clone();
    for _ in 0..MOST {
        let Some(seed) = seed(&mut open) else {
            return;
        };
        let shape = grown(&whole, &open, seed);
        open.subtract_region(&shape.region());
        pane(shape.rounded());
    }
}

fn seed(open: &mut Region<Logical>) -> Option<Step> {
    loop {
        let step = open.iter().next()?;
        if step.size.w > SLACK {
            return Some(step);
        }
        open.subtract(step);
    }
}
