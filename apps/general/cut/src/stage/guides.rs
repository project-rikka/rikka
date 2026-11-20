use rikka_app::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Guide {
    Across(f32),
    Down(f32),
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Targets {
    across: Vec<f32>,
    down: Vec<f32>,
}

impl Targets {
    pub(crate) fn new(frame: Rect, others: impl IntoIterator<Item = Rect>) -> Targets {
        let mut targets = Targets::default();
        for rect in std::iter::once(frame).chain(others) {
            targets
                .across
                .extend([rect.min_x(), rect.center().x, rect.max_x()]);
            targets
                .down
                .extend([rect.min_y(), rect.center().y, rect.max_y()]);
        }
        targets
    }

    pub(crate) fn pull(&self, moving: Rect, within: f32) -> (Point, Vec<Guide>) {
        let across = nearest(
            &[moving.min_x(), moving.center().x, moving.max_x()],
            &self.across,
            within,
        );
        let down = nearest(
            &[moving.min_y(), moving.center().y, moving.max_y()],
            &self.down,
            within,
        );
        let mut guides = Vec::new();
        if let Some((_, at)) = across {
            guides.push(Guide::Down(at));
        }
        if let Some((_, at)) = down {
            guides.push(Guide::Across(at));
        }
        (
            Point::new(
                across.map_or(0.0, |(by, _)| by),
                down.map_or(0.0, |(by, _)| by),
            ),
            guides,
        )
    }
}

fn nearest(edges: &[f32], targets: &[f32], within: f32) -> Option<(f32, f32)> {
    edges
        .iter()
        .flat_map(|edge| targets.iter().map(move |target| (target - edge, *target)))
        .filter(|(by, _)| by.abs() <= within)
        .min_by(|a, b| a.0.abs().total_cmp(&b.0.abs()))
}
