use crate::fill::edges::Edges;

pub(crate) fn clip_line(
    edges: &mut Edges,
    from: (f32, f32),
    to: (f32, f32),
    width: f32,
    height: f32,
) {
    let finite = (from.0 + from.1 + to.0 + to.1).is_finite();
    let below = from.1 <= 0.0 && to.1 <= 0.0;
    let above = from.1 >= height && to.1 >= height;
    if !finite || from.1 == to.1 || below || above {
        return;
    }
    let slope = (to.0 - from.0) / (to.1 - from.1);
    let clamp_y = |point: (f32, f32)| {
        let y = point.1.clamp(0.0, height);
        if y == point.1 {
            point
        } else {
            (from.0 + (y - from.1) * slope, y)
        }
    };
    clip_columns(edges, clamp_y(from), clamp_y(to), width);
}

fn clip_columns(edges: &mut Edges, from: (f32, f32), to: (f32, f32), width: f32) {
    if from.0 >= width && to.0 >= width {
        return;
    }
    let dx = to.0 - from.0;
    let mut cuts = [0.0f32, 1.0, 1.0, 1.0];
    let mut count = 1;
    for edge in [0.0, width] {
        let t = (edge - from.0) / dx;
        if t > 0.0 && t < 1.0 {
            cuts[count] = t;
            count += 1;
        }
    }
    cuts[count] = 1.0;
    if count == 3 && cuts[1] > cuts[2] {
        cuts.swap(1, 2);
    }
    let at = |t: f32| {
        if t == 1.0 {
            to
        } else {
            (from.0 + dx * t, from.1 + (to.1 - from.1) * t)
        }
    };
    for index in 0..count {
        let (start, end) = (at(cuts[index]), at(cuts[index + 1]));
        let middle = (start.0 + end.0) * 0.5;
        if middle >= width {
            continue;
        }
        let (x0, x1) = if middle <= 0.0 {
            (0.0, 0.0)
        } else {
            (start.0.clamp(0.0, width), end.0.clamp(0.0, width))
        };
        edges.push((x0, start.1), (x1, end.1));
    }
}
