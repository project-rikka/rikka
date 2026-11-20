use rikka_types::{CursorShape, Point, Rect};

pub const GRIP_REACH: f32 = 7.0;
pub const TURN_GAP: f32 = 24.0;

use crate::affine::Affine;
use crate::form::Form;
use crate::frame::{Frame, enclosing};

const CORNERS: [(i8, i8); 4] = [(-1, -1), (1, -1), (1, 1), (-1, 1)];
const SIDES: [(i8, i8); 4] = [(0, -1), (1, 0), (0, 1), (-1, 0)];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Grip {
    End(usize),
    Side(i8, i8),
    Turn,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Grips {
    Ends(Point, Point),
    Frame {
        frame: Frame,
        sides: bool,
        turns: bool,
    },
}

impl Grips {
    #[must_use]
    pub fn for_forms(forms: &[&Form]) -> Option<Grips> {
        let each = forms
            .iter()
            .map(|form| Grips::single(form))
            .collect::<Option<Vec<Grips>>>()?;
        Grips::union(each)
    }

    fn single(form: &Form) -> Option<Grips> {
        Some(match form {
            Form::Line { from, to, .. } | Form::Arrow { from, to, .. } => Grips::Ends(*from, *to),
            Form::Text(_) => Grips::Frame {
                frame: form.frame()?,
                sides: false,
                turns: false,
            },
            Form::Shape { .. } | Form::Path(_) => Grips::Frame {
                frame: form.frame()?,
                sides: true,
                turns: true,
            },
        })
    }

    #[must_use]
    pub fn union(all: impl IntoIterator<Item = Grips>) -> Option<Grips> {
        let mut all = all.into_iter();
        let first = all.next()?;
        let Some(second) = all.next() else {
            return Some(first);
        };
        let whole = all.fold(first.bounds().union(&second.bounds()), |whole, one| {
            whole.union(&one.bounds())
        });
        Some(Grips::Frame {
            frame: Frame::from_rect(whole),
            sides: true,
            turns: true,
        })
    }

    #[must_use]
    pub fn bounds(&self) -> Rect {
        enclosing(self.outline())
    }

    #[must_use]
    pub fn outline(&self) -> Vec<Point> {
        match self {
            Grips::Ends(from, to) => vec![*from, *to],
            Grips::Frame { frame, .. } => frame.corners().to_vec(),
        }
    }

    #[must_use]
    pub fn cursor(&self, grip: Grip) -> CursorShape {
        let Grip::Side(x, y) = grip else {
            return match grip {
                Grip::Turn => CursorShape::Grab,
                Grip::End(_) | Grip::Side(..) => CursorShape::Crosshair,
            };
        };
        let angle = match self {
            Grips::Frame { frame, .. } => frame.angle,
            Grips::Ends(..) => 0.0,
        };
        let pointing = f32::from(y).atan2(f32::from(x)).to_degrees() + angle;
        match ((pointing.rem_euclid(180.0) + 22.5) / 45.0) as u32 % 4 {
            0 => CursorShape::ResizeEastWest,
            1 => CursorShape::ResizeNorthWestSouthEast,
            2 => CursorShape::ResizeNorthSouth,
            _ => CursorShape::ResizeNorthEastSouthWest,
        }
    }

    #[must_use]
    pub fn handles(&self) -> Vec<(Grip, Point)> {
        match self {
            Grips::Ends(from, to) => vec![(Grip::End(0), *from), (Grip::End(1), *to)],
            Grips::Frame { frame, sides, .. } => {
                let spots = match sides {
                    true => [CORNERS, SIDES].concat(),
                    false => CORNERS.to_vec(),
                };
                spots
                    .into_iter()
                    .map(|(x, y)| (Grip::Side(x, y), frame.spot(x, y)))
                    .collect()
            }
        }
    }

    #[must_use]
    pub fn turn(&self, gap: f32) -> Option<(Point, Point)> {
        match self {
            Grips::Frame {
                frame, turns: true, ..
            } => {
                let top = frame.spot(0, -1);
                let (_, down) = frame.axes();
                Some((top, top - down * gap))
            }
            Grips::Ends(..) | Grips::Frame { .. } => None,
        }
    }

    #[must_use]
    pub fn contains(&self, at: Point, reach: f32) -> bool {
        match self {
            Grips::Ends(..) => false,
            Grips::Frame { frame, .. } => frame.contains(at, reach),
        }
    }

    #[must_use]
    pub fn grip_at(
        &self,
        at: Point,
        reach: f32,
        turn_gap: f32,
        shown: impl Fn(Point) -> Point,
    ) -> Option<Grip> {
        let near = |spot: Point| shown(spot).distance(at) <= reach;
        if let Some((_, turn)) = self.turn(turn_gap)
            && near(turn)
        {
            return Some(Grip::Turn);
        }
        self.handles()
            .into_iter()
            .find(|(_, spot)| near(*spot))
            .map(|(grip, _)| grip)
    }

    #[must_use]
    pub fn mapped(&self, map: Affine) -> Grips {
        match *self {
            Grips::Frame {
                frame,
                sides,
                turns,
            } => Grips::Frame {
                frame: frame.mapped(map),
                sides,
                turns,
            },
            Grips::Ends(from, to) => Grips::Ends(map.apply(from), map.apply(to)),
        }
    }
}
