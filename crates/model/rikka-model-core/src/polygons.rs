use std::ops::Range;

use crate::error::{Error, Result};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Polygons {
    pub vertex_points: Vec<u32>,
    pub offsets: Vec<u32>,
    pub corner_vertices: Vec<u32>,
    pub submeshes: Vec<u32>,
}

impl Polygons {
    #[must_use]
    pub fn len(&self) -> usize {
        self.offsets.len().saturating_sub(1)
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    #[must_use]
    pub fn range(&self, polygon: usize) -> Range<usize> {
        self.offsets[polygon] as usize..self.offsets[polygon + 1] as usize
    }

    #[must_use]
    pub fn polygon(&self, polygon: usize) -> &[u32] {
        &self.corner_vertices[self.range(polygon)]
    }

    #[must_use]
    pub fn point_count(&self) -> usize {
        self.vertex_points
            .iter()
            .max()
            .map_or(0, |&point| point as usize + 1)
    }

    pub fn flip_winding(&mut self) {
        for polygon in 0..self.len() {
            let range = self.range(polygon);
            self.corner_vertices[range.start + 1..range.end].reverse();
        }
    }

    pub(crate) fn validate(&self, mesh: &str, vertices: usize, submeshes: usize) -> Result<()> {
        let invalid = |reason: String| Err(Error::Invalid(format!("mesh \"{mesh}\": {reason}")));
        if self.vertex_points.len() != vertices {
            return invalid(format!(
                "{} polygon points for {vertices} vertices",
                self.vertex_points.len()
            ));
        }
        if self.offsets.first() != Some(&0) {
            return invalid("polygon offsets do not start at 0".to_owned());
        }
        if self.offsets.last().map(|&last| last as usize) != Some(self.corner_vertices.len()) {
            return invalid(format!(
                "polygon offsets do not end at {} corners",
                self.corner_vertices.len()
            ));
        }
        if let Some(polygon) = self
            .offsets
            .windows(2)
            .position(|pair| pair[1] < pair[0].saturating_add(3))
        {
            return invalid(format!("polygon {polygon} has fewer than 3 corners"));
        }
        if self.submeshes.len() != self.len() {
            return invalid(format!(
                "{} polygon submeshes for {} polygons",
                self.submeshes.len(),
                self.len()
            ));
        }
        if let Some(&worst) = self.corner_vertices.iter().max()
            && worst as usize >= vertices
        {
            return invalid(format!("polygon corner names vertex {worst} of {vertices}"));
        }
        if let Some(&worst) = self.submeshes.iter().max()
            && worst as usize >= submeshes
        {
            return invalid(format!("polygon names submesh {worst} of {submeshes}"));
        }
        Ok(())
    }
}
