use std::sync::Arc;

use cut_project::{Height, Project};

const SPARE: usize = 2;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Row {
    pub(crate) layer: usize,
    pub(crate) top: f32,
    pub(crate) height: f32,
}

impl Row {
    pub(crate) fn bottom(&self) -> f32 {
        self.top + self.height
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Opening {
    pub(crate) layer: usize,
    pub(crate) top: f32,
    pub(crate) height: f32,
}

impl Opening {
    fn bottom(&self) -> f32 {
        self.top + self.height
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Rows {
    tops: Arc<[f32]>,
    fallback: f32,
    opening: Option<Opening>,
}

impl Rows {
    pub(crate) fn layout(project: &Project, resizing: Option<(usize, Height)>) -> Rows {
        let count = project.layers().len() + SPARE;
        let height = |layer: usize| match resizing {
            Some((resized, height)) if resized == layer => height.px(),
            _ => project.height_of(layer).px(),
        };
        let tops: Arc<[f32]> = std::iter::once(0.0)
            .chain((0..count).scan(0.0, |top, layer| {
                *top += height(layer);
                Some(*top)
            }))
            .collect();
        Rows {
            tops,
            fallback: project.layer_height().px(),
            opening: None,
        }
    }

    pub(crate) fn with_expanded(self, layer: usize, height: f32) -> Rows {
        let opening = (layer < self.count()).then(|| Opening {
            layer,
            top: self.tops[layer + 1],
            height,
        });
        Rows { opening, ..self }
    }

    fn count(&self) -> usize {
        self.tops.len() - 1
    }

    pub(crate) fn opening(&self) -> Option<Opening> {
        self.opening
    }

    pub(crate) fn all(&self) -> impl Iterator<Item = Row> + '_ {
        (0..self.count()).map(|layer| self.layer_row(layer))
    }

    pub(crate) fn height(&self) -> f32 {
        self.tops[self.count()] + self.opening.map_or(0.0, |opening| opening.height)
    }

    pub(crate) fn at(&self, y: f32) -> Option<Row> {
        if self
            .opening
            .is_some_and(|opening| y >= opening.top && y < opening.bottom())
        {
            return None;
        }
        let layer = self.index_at(self.unopened(y));
        usize::try_from(layer)
            .ok()
            .filter(|layer| *layer < self.count())
            .map(|layer| self.layer_row(layer))
    }

    pub(crate) fn layer_row(&self, layer: usize) -> Row {
        let below = self
            .opening
            .filter(|opening| layer > opening.layer)
            .map_or(0.0, |opening| opening.height);
        let count = self.count();
        let (top, height) = match layer < count {
            true => (self.tops[layer], self.tops[layer + 1] - self.tops[layer]),
            false => (
                self.tops[count] + (layer - count) as f32 * self.fallback,
                self.fallback,
            ),
        };
        Row {
            layer,
            top: top + below,
            height,
        }
    }

    pub(crate) fn across(&self, from: f32, to: f32) -> i32 {
        let index = |y: f32| self.index_at(self.unopened(y));
        (index(to) - index(from)) as i32
    }

    fn index_at(&self, y: f32) -> i64 {
        let end = self.tops[self.count()];
        if y < 0.0 {
            return (y / self.fallback).floor() as i64;
        }
        if y >= end {
            return self.count() as i64 + ((y - end) / self.fallback).floor() as i64;
        }
        self.tops.partition_point(|top| *top <= y) as i64 - 1
    }

    fn unopened(&self, y: f32) -> f32 {
        match self.opening {
            Some(opening) if y >= opening.bottom() => y - opening.height,
            Some(opening) if y >= opening.top => {
                let row = self.layer_row(opening.layer);
                row.top + row.height / 2.0
            }
            _ => y,
        }
    }
}
