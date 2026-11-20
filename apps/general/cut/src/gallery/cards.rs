use std::f32::consts::TAU;

use cut_project::{Rate, Transition, TransitionKind};
use cut_render::Swatch;
use rikka_image_edit::{
    Depth, FillRule, Gradient, GradientInk, GradientShape, Imaging, Marking, Marks, Path, PathMark,
    Picture, Recipe, Result,
};
use rikka_types::{Color, Point};

pub(super) const WIDTH: u32 = 256;
pub(super) const HEIGHT: u32 = 144;
pub(super) const PASSING: i64 = 20;
pub(super) const HOLD: i64 = 8;
pub(super) const LOOP: i64 = HOLD + PASSING + HOLD;
pub(super) const POSTER: i64 = HOLD + PASSING * 9 / 20;
const PROJECT_WIDTH: f32 = 1920.0;
const SEED: u32 = 7;
const ROUND: usize = 48;

pub(super) struct Cards {
    imaging: Imaging,
    out: Picture,
    into: Picture,
}

impl Cards {
    pub(super) fn new(imaging: &Imaging) -> Result<Cards> {
        Ok(Cards {
            imaging: imaging.clone(),
            out: card(
                imaging,
                [Color::rgb(1.0, 0.62, 0.24), Color::rgb(0.93, 0.22, 0.43)],
                circle(
                    Point::new(WIDTH as f32 * 0.34, HEIGHT as f32 * 0.5),
                    HEIGHT as f32 * 0.24,
                ),
            )?,
            into: card(
                imaging,
                [Color::rgb(0.16, 0.76, 1.0), Color::rgb(0.36, 0.29, 1.0)],
                star(
                    Point::new(WIDTH as f32 * 0.66, HEIGHT as f32 * 0.5),
                    HEIGHT as f32 * 0.3,
                ),
            )?,
        })
    }

    pub(super) fn frame(&self, kind: TransitionKind, step: i64) -> Result<Picture> {
        match step {
            step if step < HOLD => Ok(self.out.clone()),
            step if step >= HOLD + PASSING => Ok(self.into.clone()),
            step => Swatch::new(&self.imaging, Transition::new(kind, Rate::VIDEO))
                .frames(PASSING)
                .scale(WIDTH as f32 / PROJECT_WIDTH)
                .seed(SEED)
                .render(step - HOLD, &self.out, &self.into),
        }
    }
}

fn card(imaging: &Imaging, [from, to]: [Color; 2], figure: Path) -> Result<Picture> {
    let (width, height) = (WIDTH as f32, HEIGHT as f32);
    let stripes = (0..4).map(|index| {
        let left = width * (0.08 + 0.24 * index as f32);
        Marking::from(
            PathMark::new(Path::polygon([
                Point::new(left, height),
                Point::new(left + width * 0.06, height),
                Point::new(left + width * 0.26, 0.0),
                Point::new(left + width * 0.2, 0.0),
            ]))
            .filled(Color::rgba(1.0, 1.0, 1.0, 0.16), FillRule::NonZero),
        )
    });
    let marks = Marks::new(stripes.chain([Marking::from(
        PathMark::new(figure).filled(Color::WHITE, FillRule::NonZero),
    )]));
    imaging.blank(WIDTH, HEIGHT, Depth::Eight, from)?.develop(
        &Recipe::new()
            .then(Gradient::new(
                GradientShape::Linear,
                Point::new(0.0, 0.0),
                Point::new(width, height),
                GradientInk::Colors { from, to },
            ))
            .then(marks),
    )
}

fn circle(centre: Point, radius: f32) -> Path {
    Path::polygon((0..ROUND).map(|step| {
        let turn = step as f32 / ROUND as f32 * TAU;
        Point::new(
            centre.x + radius * turn.cos(),
            centre.y + radius * turn.sin(),
        )
    }))
}

fn star(centre: Point, radius: f32) -> Path {
    Path::polygon((0..10).map(|step| {
        let reach = match step % 2 {
            0 => radius,
            _ => radius * 0.45,
        };
        let turn = step as f32 / 10.0 * TAU - TAU / 4.0;
        Point::new(centre.x + reach * turn.cos(), centre.y + reach * turn.sin())
    }))
}
