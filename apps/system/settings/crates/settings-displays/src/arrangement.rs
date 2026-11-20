use crate::bounds::Bounds;

const SNAP_DISTANCE: i32 = 160;

pub fn snap(dropped: Bounds, others: &[Bounds]) -> (i32, i32) {
    let mut snapped = dropped;
    snapped.x += nearest(
        others
            .iter()
            .flat_map(|other| alignments(snapped.x, snapped.width, other.x, other.width)),
    );
    snapped.y += nearest(
        others
            .iter()
            .flat_map(|other| alignments(snapped.y, snapped.height, other.y, other.height)),
    );
    for other in others {
        if snapped.overlaps(*other) {
            snapped = push_clear(snapped, *other);
        }
    }
    (snapped.x, snapped.y)
}

pub fn union(bounds: impl IntoIterator<Item = Bounds>) -> Bounds {
    let mut bounds = bounds.into_iter();
    let Some(first) = bounds.next() else {
        return Bounds {
            x: 0,
            y: 0,
            width: 1,
            height: 1,
        };
    };
    let mut union = first;
    for other in bounds {
        let right = union.right().max(other.right());
        let bottom = union.bottom().max(other.bottom());
        union.x = union.x.min(other.x);
        union.y = union.y.min(other.y);
        union.width = right - union.x;
        union.height = bottom - union.y;
    }
    union
}

fn alignments(start: i32, length: i32, other: i32, other_length: i32) -> [i32; 6] {
    [
        other - (start + length),
        (other + other_length) - start,
        other - start,
        (other + other_length) - (start + length),
        other - (start + length - other_length),
        (other + (other_length - length) / 2) - start,
    ]
}

fn nearest(candidates: impl Iterator<Item = i32>) -> i32 {
    candidates
        .filter(|offset| offset.abs() <= SNAP_DISTANCE)
        .min_by_key(|offset| offset.abs())
        .unwrap_or(0)
}

fn push_clear(moving: Bounds, fixed: Bounds) -> Bounds {
    let left = fixed.x - moving.right();
    let right = fixed.right() - moving.x;
    let up = fixed.y - moving.bottom();
    let down = fixed.bottom() - moving.y;

    let (horizontal, vertical) = (
        if left.abs() < right.abs() {
            left
        } else {
            right
        },
        if up.abs() < down.abs() { up } else { down },
    );
    if horizontal.abs() <= vertical.abs() {
        Bounds {
            x: moving.x + horizontal,
            ..moving
        }
    } else {
        Bounds {
            y: moving.y + vertical,
            ..moving
        }
    }
}
