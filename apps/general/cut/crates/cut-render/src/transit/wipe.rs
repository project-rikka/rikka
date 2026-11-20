use cut_project::{Direction, TransitionKind as Kind};
use rikka_image_edit::{FillRule, Mask, Path, Picture, Result};
use rikka_types::Point;

use super::Mix;

pub(super) fn render(mix: &Mix, out: &Picture, into: &Picture) -> Result<Picture> {
    let progress = mix.progress;
    if progress <= 0.0 {
        return Ok(out.clone());
    }
    if progress >= 1.0 {
        return Ok(into.clone());
    }
    let feather = mix.tuning.softness / 100.0 * mix.reach() * 0.5;
    let mask = match mix.kind {
        Kind::Split => split(mix, feather),
        Kind::IrisRound => iris(mix, feather, true),
        Kind::IrisBox => iris(mix, feather, false),
        _ => wipe(mix, feather),
    };
    let mask = match feather > 0.0 {
        true => mask.feathered(feather),
        false => mask,
    };
    mix.mixed(out, into, 1.0, Some(mask))
}

fn wipe(mix: &Mix, feather: f32) -> Mask {
    let (cx, cy) = mix.centre();
    let centre = Point::new(cx, cy);
    let along = mix.direction.vector();
    let across = Point::new(-along.y, along.x);
    let corners = [
        Point::new(0.0, 0.0),
        Point::new(mix.width as f32, 0.0),
        Point::new(0.0, mix.height as f32),
        Point::new(mix.width as f32, mix.height as f32),
    ];
    let reach = corners
        .iter()
        .map(|corner| ((corner.x - cx) * along.x + (corner.y - cy) * along.y).abs())
        .fold(0.0f32, f32::max);
    let edge = -reach - feather + mix.progress * 2.0 * (reach + feather);
    let far = mix.reach() * 2.0;
    let point = |forward: f32, side: f32| {
        Point::new(
            centre.x + along.x * forward + across.x * side,
            centre.y + along.y * forward + across.y * side,
        )
    };
    Mask::path(
        Path::polygon([
            point(-far, -far),
            point(edge, -far),
            point(edge, far),
            point(-far, far),
        ]),
        FillRule::NonZero,
    )
}

fn split(mix: &Mix, feather: f32) -> Mask {
    let (cx, cy) = mix.centre();
    let (width, height) = (mix.width as f32, mix.height as f32);
    match mix.direction {
        Direction::Up | Direction::Down => {
            let open = -feather + mix.progress * (height / 2.0 + 2.0 * feather);
            Mask::rectangle(-feather, cy - open, width + 2.0 * feather, 2.0 * open)
        }
        _ => {
            let open = -feather + mix.progress * (width / 2.0 + 2.0 * feather);
            Mask::rectangle(cx - open, -feather, 2.0 * open, height + 2.0 * feather)
        }
    }
}

fn iris(mix: &Mix, feather: f32, round: bool) -> Mask {
    let (cx, cy) = mix.centre();
    let radius_at = |reach: f32| -feather + mix.progress * (reach + 2.0 * feather);
    match round {
        true => {
            let radius = radius_at(cx.hypot(cy));
            Mask::ellipse(cx - radius, cy - radius, 2.0 * radius, 2.0 * radius)
        }
        false => {
            let (across, down) = (radius_at(cx), radius_at(cy));
            Mask::rectangle(cx - across, cy - down, 2.0 * across, 2.0 * down)
        }
    }
}
