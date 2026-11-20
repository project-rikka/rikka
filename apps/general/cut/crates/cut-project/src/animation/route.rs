use super::cubic::{self, Cubic};
use super::key::Key;
use super::kind::{Coords, Kind};

const STRAIGHT: f64 = 1e-6;
const NOWHERE: f64 = 1e-9;
const PIECES: usize = 4;
const NODES: [(f64, f64); 8] = [
    (-0.960_289_856_497_536_3, 0.101_228_536_290_376_3),
    (-0.796_666_477_413_626_7, 0.222_381_034_453_374_5),
    (-0.525_532_409_916_329, 0.313_706_645_877_887_3),
    (-0.183_434_642_495_649_8, 0.362_683_783_378_362),
    (0.183_434_642_495_649_8, 0.362_683_783_378_362),
    (0.525_532_409_916_329, 0.313_706_645_877_887_3),
    (0.796_666_477_413_626_7, 0.222_381_034_453_374_5),
    (0.960_289_856_497_536_3, 0.101_228_536_290_376_3),
];

pub(crate) type Points = [[f64; 2]; 4];

#[derive(Clone, Copy, Debug)]
pub(crate) enum Route {
    Line {
        from: Coords,
        to: Coords,
        length: f64,
        signed: bool,
    },
    Curve(Arc),
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Arc {
    xs: Cubic,
    ys: Cubic,
    length: f64,
}

impl Route {
    pub(crate) fn between<K: Kind>(from: &Key<K>, to: &Key<K>) -> Route {
        let (start, end) = (K::coords(from.value), K::coords(to.value));
        match bezier::<K>(from, to) {
            Some(points) => Route::spatial(points),
            None if K::DIMENSIONS == 1 => Route::Line {
                from: start,
                to: end,
                length: end[0] - start[0],
                signed: true,
            },
            None => Route::Line {
                from: start,
                to: end,
                length: distance(&start, &end),
                signed: false,
            },
        }
    }

    pub(crate) fn spatial(points: Points) -> Route {
        let line = |length: f64| Route::Line {
            from: [points[0][0], points[0][1], 0.0, 0.0],
            to: [points[3][0], points[3][1], 0.0, 0.0],
            length,
            signed: false,
        };
        let chord = sub(points[3], points[0]);
        let length = chord[0].hypot(chord[1]);
        if length < NOWHERE {
            return match points.iter().all(|point| {
                let away = sub(*point, points[0]);
                away[0].hypot(away[1]) < NOWHERE
            }) {
                true => line(0.0),
                false => Route::Curve(Arc::new(points)),
            };
        }
        let along = |point: [f64; 2]| {
            let away = sub(point, points[0]);
            let across = (chord[0] * away[1] - chord[1] * away[0]).abs() / length;
            let ahead = (chord[0] * away[0] + chord[1] * away[1]) / length;
            let slack = STRAIGHT * length.max(1.0);
            across <= slack && ahead >= -slack && ahead <= length + slack
        };
        match along(points[1]) && along(points[2]) {
            true => line(length),
            false => Route::Curve(Arc::new(points)),
        }
    }

    pub(crate) fn length(&self) -> f64 {
        match self {
            Route::Line { length, .. } => *length,
            Route::Curve(arc) => arc.length,
        }
    }

    pub(crate) fn at(&self, travel: f64) -> Coords {
        match self {
            Route::Line {
                from, signed: true, ..
            } => [from[0] + travel, from[1], from[2], from[3]],
            Route::Line {
                from, to, length, ..
            } => match *length > NOWHERE {
                true => std::array::from_fn(|axis| {
                    from[axis] + (to[axis] - from[axis]) * travel / length
                }),
                false => *from,
            },
            Route::Curve(arc) => {
                let [x, y] = arc.at(travel);
                [x, y, 0.0, 0.0]
            }
        }
    }
}

impl Arc {
    fn new(points: Points) -> Arc {
        let mut arc = Arc {
            xs: points.map(|point| point[0]),
            ys: points.map(|point| point[1]),
            length: 0.0,
        };
        arc.length = arc.length_to(1.0);
        arc
    }

    pub(crate) fn points(&self) -> Points {
        std::array::from_fn(|index| [self.xs[index], self.ys[index]])
    }

    pub(crate) fn param(&self, travel: f64) -> f64 {
        if travel <= 0.0 {
            return 0.0;
        }
        if travel >= self.length {
            return 1.0;
        }
        let tolerance = 1e-10 * self.length.max(1.0);
        let (mut low, mut high) = (0.0, 1.0);
        let mut u = travel / self.length;
        for _ in 0..60 {
            let error = self.length_to(u) - travel;
            if error.abs() <= tolerance {
                break;
            }
            match error > 0.0 {
                true => high = u,
                false => low = u,
            }
            let rate = self.speed(u);
            let guess = u - error / rate;
            u = match rate > 0.0 && guess > low && guess < high {
                true => guess,
                false => (low + high) / 2.0,
            };
        }
        u
    }

    fn at(&self, travel: f64) -> [f64; 2] {
        if travel < 0.0 {
            let [x, y] = self.heading(0.0);
            return [self.xs[0] + x * travel, self.ys[0] + y * travel];
        }
        if travel > self.length {
            let [x, y] = self.heading(1.0);
            let beyond = travel - self.length;
            return [self.xs[3] + x * beyond, self.ys[3] + y * beyond];
        }
        let u = self.param(travel);
        [cubic::at(self.xs, u), cubic::at(self.ys, u)]
    }

    fn heading(&self, u: f64) -> [f64; 2] {
        let points = self.points();
        let candidates = match u < 0.5 {
            true => [
                [cubic::slope(self.xs, u), cubic::slope(self.ys, u)],
                sub(points[2], points[0]),
                sub(points[3], points[0]),
            ],
            false => [
                [cubic::slope(self.xs, u), cubic::slope(self.ys, u)],
                sub(points[3], points[1]),
                sub(points[3], points[0]),
            ],
        };
        candidates
            .into_iter()
            .find_map(|[x, y]| {
                let length = x.hypot(y);
                (length > NOWHERE).then(|| [x / length, y / length])
            })
            .unwrap_or([0.0, 0.0])
    }

    fn speed(&self, u: f64) -> f64 {
        cubic::slope(self.xs, u).hypot(cubic::slope(self.ys, u))
    }

    fn length_to(&self, u: f64) -> f64 {
        let step = u / PIECES as f64;
        (0..PIECES)
            .map(|piece| {
                let middle = step * (piece as f64 + 0.5);
                NODES
                    .iter()
                    .map(|(node, weight)| weight * self.speed(middle + node * step / 2.0))
                    .sum::<f64>()
                    * step
                    / 2.0
            })
            .sum()
    }
}

pub(crate) fn bezier<K: Kind>(from: &Key<K>, to: &Key<K>) -> Option<Points> {
    let (start, end) = (K::coords(from.value), K::coords(to.value));
    let (start_bend, end_bend) = (K::bend(&from.bend)?, K::bend(&to.bend)?);
    let (_, outgoing) = start_bend.tangents();
    let (incoming, _) = end_bend.tangents();
    Some([
        [start[0], start[1]],
        [
            start[0] + f64::from(outgoing.x),
            start[1] + f64::from(outgoing.y),
        ],
        [
            end[0] + f64::from(incoming.x),
            end[1] + f64::from(incoming.y),
        ],
        [end[0], end[1]],
    ])
}

pub(crate) fn distance(from: &Coords, to: &Coords) -> f64 {
    from.iter()
        .zip(to)
        .map(|(from, to)| (to - from).powi(2))
        .sum::<f64>()
        .sqrt()
}

fn sub(left: [f64; 2], right: [f64; 2]) -> [f64; 2] {
    [left[0] - right[0], left[1] - right[1]]
}
