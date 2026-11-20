use rikka_types::Rect;
use smallvec::SmallVec;

use crate::draw::chunks::{Chunk, Run, Segment};
use crate::scene::{INVISIBLE, LayerId, Node, NodeId, Scene};

#[derive(Clone, Copy, Debug)]
struct OpenFade {
    id: LayerId,
    opacity: f32,
    capture: Option<usize>,
    covered: Rect,
}

#[derive(Debug, Default)]
pub(crate) struct Fades {
    open: Vec<OpenFade>,
    covered: Vec<(LayerId, Rect)>,
}

impl Fades {
    pub(crate) fn reset(&mut self) {
        self.open.clear();
        self.covered.clear();
    }

    pub(crate) fn settle_bounds(&mut self, scene: &mut Scene) {
        self.covered.sort_unstable_by_key(|(layer, _)| *layer);
        for runs in self.covered.chunk_by(|(left, _), (right, _)| left == right) {
            let bounds = runs
                .iter()
                .fold(Rect::ZERO, |bounds, (_, covered)| bounds.union(covered));
            scene.set_layer_bounds(runs[0].0, bounds);
        }
        self.covered.clear();
    }

    pub(crate) fn enter(
        &mut self,
        scene: &Scene,
        node: &Node,
        bounds: Rect,
        run: &mut Run,
        chunks: &mut Vec<Chunk>,
        segments: &mut Vec<Segment>,
    ) -> bool {
        if node.layer.is_none() && self.open.is_empty() {
            return true;
        }
        let chain = scene.layer_chain(node.layer);
        if chain.iter().any(|(_, opacity)| *opacity <= INVISIBLE) {
            let region = bounds.round_out();
            for &(id, _) in chain {
                self.covered.push((id, region));
            }
            return false;
        }
        let shared = self
            .open
            .iter()
            .zip(chain.iter())
            .take_while(|(open, (id, _))| open.id == *id)
            .count();
        while self.open.len() > shared {
            self.close(run, chunks, segments);
        }
        for &(id, opacity) in &chain[shared..] {
            let capture = (opacity < 1.0).then(|| {
                run.flush(chunks, segments);
                segments.push(Segment::Capture { region: Rect::ZERO });
                segments.len() - 1
            });
            self.open.push(OpenFade {
                id,
                opacity,
                capture,
                covered: Rect::ZERO,
            });
        }
        for open in &mut self.open {
            open.covered = open.covered.union(&bounds);
        }
        true
    }

    pub(crate) fn close_all(
        &mut self,
        run: &mut Run,
        chunks: &mut Vec<Chunk>,
        segments: &mut Vec<Segment>,
    ) {
        while !self.open.is_empty() {
            self.close(run, chunks, segments);
        }
    }

    fn close(&mut self, run: &mut Run, chunks: &mut Vec<Chunk>, segments: &mut Vec<Segment>) {
        let Some(open) = self.open.pop() else { return };
        let region = open.covered.round_out();
        if let Some(at) = open.capture {
            run.flush(chunks, segments);
            segments[at] = Segment::Capture { region };
            segments.push(Segment::Fade {
                layer: open.id,
                opacity: open.opacity,
            });
        }
        self.covered.push((open.id, region));
    }
}

pub(crate) fn update_fades(scene: &Scene, segments: &mut [Segment]) {
    for segment in segments {
        if let Segment::Fade { layer, opacity } = segment
            && let Some(now) = scene.layer_opacity(*layer)
        {
            *opacity = now;
        }
    }
}

pub(crate) fn reframe(scene: &mut Scene, segments: &mut [Segment], nodes: &[NodeId]) {
    let mut widened = false;
    for &id in nodes {
        let Some(node) = scene.get(id).filter(|node| node.paints()) else {
            continue;
        };
        let bounds = scene.bounds_of(node);
        if node.backdrop.is_some() {
            let placed = node.rect.translate(scene.offset_of(node.group));
            for segment in segments.iter_mut() {
                if let Segment::Backdrop { node, region, .. } = segment
                    && *node == id
                {
                    *region = placed;
                }
            }
        }
        let chain: SmallVec<[LayerId; 4]> = scene
            .layer_chain(node.layer)
            .iter()
            .map(|(layer, _)| *layer)
            .collect();
        for layer in chain {
            widened |= scene.widen_layer_bounds(layer, bounds.round_out());
        }
    }
    if !widened {
        return;
    }
    let mut open: SmallVec<[usize; 4]> = SmallVec::new();
    for at in 0..segments.len() {
        match segments[at] {
            Segment::Capture { .. } => open.push(at),
            Segment::Fade { layer, .. } => {
                if let (Some(capture), Some(bounds)) = (open.pop(), scene.layer_bounds(layer)) {
                    segments[capture] = Segment::Capture { region: bounds };
                }
            }
            _ => {}
        }
    }
}
