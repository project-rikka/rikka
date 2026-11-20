use rikka_image_edit::{GradientMap, GradientStop};
use rikka_types::Color;
use serde::{Deserialize, Serialize};

use crate::hex;

pub(crate) const FEWEST_STOPS: usize = 2;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(from = "Vec<Stop>", into = "Vec<Stop>")]
pub struct Gradient {
    stops: Vec<GradientStop>,
}

#[derive(Serialize, Deserialize)]
struct Stop {
    at: f32,
    #[serde(with = "hex")]
    color: Color,
    #[serde(default = "Stop::middle")]
    midpoint: f32,
}

impl Stop {
    fn middle() -> f32 {
        0.5
    }
}

impl From<Vec<Stop>> for Gradient {
    fn from(stops: Vec<Stop>) -> Gradient {
        let stops: Vec<GradientStop> = stops
            .into_iter()
            .filter(|stop| stop.at.is_finite() && stop.midpoint.is_finite())
            .map(|stop| GradientStop::new(stop.at, stop.color).midpoint(stop.midpoint))
            .collect();
        match stops.len() >= FEWEST_STOPS {
            true => Gradient::sorted(stops),
            false => Gradient::default(),
        }
    }
}

impl From<Gradient> for Vec<Stop> {
    fn from(gradient: Gradient) -> Vec<Stop> {
        gradient
            .stops
            .into_iter()
            .map(|stop| Stop {
                at: stop.at,
                color: stop.color,
                midpoint: stop.midpoint,
            })
            .collect()
    }
}

impl Default for Gradient {
    fn default() -> Gradient {
        Gradient::from_stops(&[(0.0, Color::BLACK), (1.0, Color::WHITE)])
    }
}

impl Gradient {
    pub fn from_stops(stops: &[(f32, Color)]) -> Gradient {
        Gradient::sorted(
            stops
                .iter()
                .map(|&(at, color)| GradientStop::new(at, color))
                .collect(),
        )
    }

    fn sorted(mut stops: Vec<GradientStop>) -> Gradient {
        stops.sort_by(|a, b| a.at.total_cmp(&b.at));
        Gradient { stops }
    }

    pub fn stops(&self) -> &[GradientStop] {
        &self.stops
    }

    pub fn map(&self) -> GradientMap {
        GradientMap::new(self.stops.iter().copied())
    }

    pub fn color_at(&self, at: f32) -> Color {
        self.map().color_at(at)
    }

    pub fn insert(&mut self, at: f32) -> usize {
        let at = at.clamp(0.0, 1.0);
        let stop = GradientStop::new(at, self.color_at(at));
        let index = self.stops.partition_point(|stop| stop.at <= at);
        self.stops.insert(index, stop);
        index
    }

    pub fn place(stops: &[GradientStop], index: usize, at: f32) -> usize {
        stops
            .iter()
            .enumerate()
            .filter(|&(other, stop)| {
                other != index && (stop.at < at || (stop.at == at && other < index))
            })
            .count()
    }

    pub fn moved(&mut self, index: usize, at: f32) -> usize {
        if index >= self.stops.len() {
            return index;
        }
        let at = at.clamp(0.0, 1.0);
        let placed = Gradient::place(&self.stops, index, at);
        let mut stop = self.stops.remove(index);
        stop.at = at;
        self.stops.insert(placed, stop);
        placed
    }

    pub fn removable(stops: &[GradientStop]) -> bool {
        stops.len() > FEWEST_STOPS
    }

    pub fn remove(&mut self, index: usize) -> bool {
        let removable = Gradient::removable(&self.stops) && index < self.stops.len();
        if removable {
            self.stops.remove(index);
        }
        removable
    }

    pub fn set_color(&mut self, index: usize, color: Color) {
        if let Some(stop) = self.stops.get_mut(index) {
            stop.color = color;
        }
    }

    pub fn set_midpoint(&mut self, index: usize, midpoint: f32) {
        if let Some(stop) = self.stops.get_mut(index) {
            *stop = stop.midpoint(midpoint);
        }
    }
}
