use cut_project::{Clip, ClipId, Project, Property};
use rikka_app::prelude::*;

pub(crate) const SUMMARY: f32 = 30.0;
pub(crate) const LANE: f32 = 22.0;
const GRAPH: f32 = 220.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Lane {
    Summary,
    Property(usize),
    Graph,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Outline {
    pub(crate) clip: ClipId,
    pub(crate) layer: usize,
    pub(crate) properties: Vec<Property>,
    pub(crate) graph: bool,
}

impl Outline {
    pub(crate) fn for_clip(project: &Project, clip: &Clip, graph: bool) -> Option<Outline> {
        let (layer, _) = project.locate(clip.id())?;
        Some(Outline {
            clip: clip.id(),
            layer,
            properties: clip
                .properties()
                .into_iter()
                .filter(|property| clip.is_keyed(property))
                .collect(),
            graph,
        })
    }

    pub(crate) fn listed(&self) -> f32 {
        self.properties.len() as f32 * LANE
    }

    pub(crate) fn body(&self) -> f32 {
        match self.graph {
            true => self.listed().max(GRAPH),
            false => self.listed(),
        }
    }

    pub(crate) fn height(&self) -> f32 {
        SUMMARY + self.body()
    }

    pub(crate) fn lane_top(index: usize) -> f32 {
        SUMMARY + index as f32 * LANE
    }

    pub(crate) fn lane_at(&self, y: f32) -> Option<Lane> {
        if !(0.0..self.height()).contains(&y) {
            return None;
        }
        if y < SUMMARY {
            return Some(Lane::Summary);
        }
        if self.graph {
            return Some(Lane::Graph);
        }
        let index = ((y - SUMMARY) / LANE).floor() as usize;
        (index < self.properties.len()).then_some(Lane::Property(index))
    }

    pub(crate) fn listed_at(&self, y: f32) -> Option<usize> {
        let index = ((y - SUMMARY) / LANE).floor();
        (y >= SUMMARY && (index as usize) < self.properties.len()).then_some(index as usize)
    }

    pub(crate) fn graph_area(&self, width: f32) -> Rect {
        Rect::new(0.0, SUMMARY, width, self.body())
    }
}
