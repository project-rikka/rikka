use rikka_sync::Mutex;
use std::sync::Arc;

use glam::Vec3;
use rikka_gpu::Compute;
use rikka_gpu::ComputeBuffer;
use rikka_model_core::NodeId;
use rikka_physics_gpu::Ring;
use rikka_physics_tailor::{ClothAsset, MAX_GRABS, Style};

use crate::colliders::{Fitted, plane_count};
use crate::drape::Outputs;
use crate::error::Result;
use crate::grab::{GrabState, MAX_GRABBED};
use crate::inputs::{Joint, Posture};
use crate::launch::Extent;
use crate::layout::{
    CONTACT_WORDS, ColliderRecord, GEOMETRY_VECS, NEAREST_WORDS, PieceRecord, Sections, Topology,
    WRAP_WORDS, bake,
};

pub(crate) const VEC4: u64 = 16;
const MAT4: u64 = 64;
const WORD: u64 = 4;
const SOURCE_BYTES: u64 = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FigureId(pub(crate) u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Scratch {
    pub(crate) starts: u64,
    pub(crate) preds: u64,
    pub(crate) geometry: u64,
    pub(crate) targets: u64,
    pub(crate) slots: u64,
    pub(crate) selfs: u64,
    pub(crate) wraps: u64,
    pub(crate) nearest: u64,
    pub(crate) size: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Posed {
    pub(crate) epoch_start: u64,
    pub(crate) epoch_motion: u64,
    pub(crate) body: u64,
    pub(crate) size: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct FrameLayout {
    pub(crate) palettes: u64,
    pub(crate) pieces: u64,
    pub(crate) colliders: u64,
    pub(crate) planes: u64,
    pub(crate) grabs: u64,
    pub(crate) step_plan: u64,
    pub(crate) grabbed: u64,
    pub(crate) outputs: u64,
    pub(crate) sources: u64,
    pub(crate) size: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Motion {
    pub(crate) joints: Option<Vec<Joint>>,
    pub(crate) smoothed: Vec<Vec3>,
    pub(crate) fresh: bool,
}

pub(crate) struct Figure {
    pub(crate) id: FigureId,
    pub(crate) asset: Arc<ClothAsset>,
    pub(crate) styles: Vec<Style>,
    pub(crate) topology: Topology,
    pub(crate) sections: Sections,
    pub(crate) statics: ComputeBuffer,
    pub(crate) state: ComputeBuffer,
    pub(crate) contacts: ComputeBuffer,
    pub(crate) normals: ComputeBuffer,
    pub(crate) scratch: ComputeBuffer,
    pub(crate) scratch_layout: Scratch,
    pub(crate) posed: ComputeBuffer,
    pub(crate) posed_layout: Posed,
    pub(crate) frame: ComputeBuffer,
    pub(crate) frame_layout: FrameLayout,
    pub(crate) hands: ComputeBuffer,
    pub(crate) bound: Vec<(NodeId, u32)>,
    pub(crate) inverse_rest: Vec<glam::Mat4>,
    pub(crate) colliders: Vec<Fitted>,
    pub(crate) planes: usize,
    pub(crate) motion: Motion,
    pub(crate) shown: Vec<glam::Mat4>,
    pub(crate) posture: Option<Posture>,
    pub(crate) grabbing: GrabState,
    pub(crate) outputs: Option<Outputs>,
    pub(crate) captures: Mutex<Ring>,
}

impl Figure {
    pub(crate) fn new(
        compute: &Compute,
        id: FigureId,
        asset: Arc<ClothAsset>,
        styles: Vec<Style>,
        boundaries: u32,
    ) -> Result<Figure> {
        let topology = Topology::build(&asset);
        let count = u64::from(topology.count());
        let body = asset.body.vertices.len() as u64;
        Extent {
            particles: count,
            triangles: topology.triangles.len() as u64,
            body_vertices: body,
            body_triangles: asset.body.triangles.len() as u64,
            binds: topology.binds.len() as u64,
            boundaries: u64::from(boundaries),
        }
        .launchable(compute.widest_launch())?;
        let (bytes, sections) = bake(&topology, &asset, &styles);
        let statics = compute.buffer(bytes.len() as u64)?;
        let state = compute.buffer(3 * count * VEC4)?;
        let contacts = compute.buffer((count * CONTACT_WORDS * WORD).max(VEC4))?;
        let normals = compute.buffer(count * VEC4)?;
        let scratch_layout = scratch(
            count,
            topology.triangles.len() as u64,
            u64::from(topology.bases.total),
            u64::from(boundaries),
        );
        let scratch = compute.buffer(scratch_layout.size)?;
        let posed_layout = posed(count, body, u64::from(boundaries));
        let posed = compute.buffer(posed_layout.size)?;
        let colliders = Fitted::fit(&asset, &topology);
        let planes = plane_count(&colliders);
        let frame_layout = frame(
            topology.slots.len() as u64,
            asset.pieces.len() as u64,
            topology.nodes.len() as u64,
            (colliders.len() as u64, planes as u64),
            u64::from(boundaries),
        );
        let frame = compute.buffer(frame_layout.size)?;
        let mut batch = compute.batch();
        batch.write(&statics, 0, &bytes);
        batch.fill(&contacts, u32::MAX);
        batch.fill(&state, 0);
        batch.fill(&normals, 0);
        batch.fill(&scratch, 0);
        batch.fill(&posed, 0);
        batch.fill(&frame, 0);
        batch.submit()?;
        let hands = compute.buffer((topology.binds.len() as u64 * 4).max(VEC4))?;
        let bound = topology
            .binds
            .iter()
            .map(|&(piece, bind)| {
                let bind = asset.pieces[piece].bindings[bind];
                (bind.node, bind.vertex)
            })
            .collect();
        let inverse_rest = topology
            .slots
            .iter()
            .map(|node| asset.rest_worlds[node.index()].inverse())
            .collect();
        let pieces = asset.pieces.len();
        Ok(Figure {
            id,
            asset,
            styles,
            topology,
            sections,
            statics,
            state,
            contacts,
            normals,
            scratch,
            scratch_layout,
            posed,
            posed_layout,
            frame,
            frame_layout,
            hands,
            bound,
            inverse_rest,
            colliders,
            planes,
            motion: Motion {
                joints: None,
                smoothed: vec![Vec3::ZERO; pieces],
                fresh: true,
            },
            shown: Vec::new(),
            posture: None,
            grabbing: GrabState::default(),
            outputs: None,
            captures: Mutex::new(Ring::new(count * VEC4 + count * NEAREST_WORDS * WORD)),
        })
    }

    pub(crate) fn restyle(&mut self, compute: &Compute) -> Result<()> {
        let (bytes, sections) = bake(&self.topology, &self.asset, &self.styles);
        let mut batch = compute.batch();
        batch.write(&self.statics, 0, &bytes);
        batch.submit()?;
        self.sections = sections;
        Ok(())
    }

    pub(crate) fn count(&self) -> u32 {
        self.topology.count()
    }

    pub(crate) fn velocities(&self) -> u64 {
        u64::from(self.count()) * VEC4
    }

    pub(crate) fn drift(&self) -> u64 {
        2 * u64::from(self.count()) * VEC4
    }

    pub(crate) fn body_vertices(&self) -> u32 {
        self.asset.body.vertices.len() as u32
    }

    pub(crate) fn body_triangles(&self) -> u32 {
        self.asset.body.triangles.len() as u32
    }

    pub(crate) fn body_at(&self, boundary: u32) -> u64 {
        self.posed.address() + self.body_offset(boundary)
    }

    pub(crate) fn body_offset(&self, boundary: u32) -> u64 {
        self.posed_layout.body + u64::from(boundary) * u64::from(self.body_vertices()) * VEC4
    }

    pub(crate) fn targets_at(&self, boundary: u32) -> u64 {
        self.scratch.address()
            + self.scratch_layout.targets
            + u64::from(boundary) * u64::from(self.count()) * VEC4
    }

    pub(crate) fn colliders_offset(&self, boundary: u32) -> u64 {
        self.frame_layout.colliders
            + u64::from(boundary) * self.colliders.len() as u64 * size_of::<ColliderRecord>() as u64
    }

    pub(crate) fn planes_offset(&self, boundary: u32) -> u64 {
        self.frame_layout.planes + u64::from(boundary) * self.planes as u64 * VEC4
    }

    pub(crate) fn colliders_at(&self, boundary: u32) -> u64 {
        self.frame.address() + self.colliders_offset(boundary)
    }

    pub(crate) fn planes_at(&self, boundary: u32) -> u64 {
        self.frame.address() + self.planes_offset(boundary)
    }

    pub(crate) fn grabs_at(&self, boundary: u32) -> u64 {
        self.frame.address()
            + self.frame_layout.grabs
            + u64::from(boundary) * MAX_GRABS as u64 * VEC4
    }

    pub(crate) fn grabbed(&self) -> u64 {
        self.frame.address() + self.frame_layout.grabbed
    }

    pub(crate) fn scratch_at(&self, offset: u64) -> u64 {
        self.scratch.address() + offset
    }

    pub(crate) fn posed_at(&self, offset: u64) -> u64 {
        self.posed.address() + offset
    }

    pub(crate) fn statics_at(&self, offset: u64) -> u64 {
        self.statics.address() + offset
    }
}

#[derive(Default)]
struct BufferCarver {
    size: u64,
}

impl BufferCarver {
    fn take(&mut self, bytes: u64) -> u64 {
        let at = self.size;
        self.size += bytes.max(VEC4).next_multiple_of(256);
        at
    }
}

fn scratch(count: u64, triangles: u64, slots: u64, boundaries: u64) -> Scratch {
    let mut carver = BufferCarver::default();
    Scratch {
        starts: carver.take(count * VEC4),
        preds: carver.take(count * VEC4),
        geometry: carver.take(count * GEOMETRY_VECS * VEC4),
        targets: carver.take(boundaries * count * VEC4),
        slots: carver.take(slots * VEC4),
        selfs: carver.take(count * VEC4),
        wraps: carver.take(triangles * WRAP_WORDS * WORD),
        nearest: carver.take(count * NEAREST_WORDS * WORD),
        size: carver.size,
    }
}

fn posed(count: u64, body: u64, boundaries: u64) -> Posed {
    let mut carver = BufferCarver::default();
    let epoch_start = carver.take(2 * count * VEC4);
    Posed {
        epoch_start,
        epoch_motion: epoch_start + count * VEC4,
        body: carver.take(boundaries * body * VEC4),
        size: carver.size,
    }
}

fn frame(
    slots: u64,
    pieces: u64,
    nodes: u64,
    (colliders, planes): (u64, u64),
    boundaries: u64,
) -> FrameLayout {
    let mut carver = BufferCarver::default();
    let palettes = carver.take(boundaries * slots * MAT4);
    let pieces = carver.take(pieces * size_of::<PieceRecord>() as u64);
    let colliders = carver.take(boundaries * colliders * size_of::<ColliderRecord>() as u64);
    let planes = carver.take(boundaries * planes * VEC4);
    let grabs = carver.take(boundaries * MAX_GRABS as u64 * VEC4);
    let grabbed = carver.take(MAX_GRABBED as u64 * VEC4);
    let outputs = carver.take(nodes * 8);
    FrameLayout {
        palettes,
        pieces,
        colliders,
        planes,
        grabs,
        step_plan: grabbed,
        grabbed,
        outputs,
        sources: carver.take(nodes * SOURCE_BYTES),
        size: carver.size,
    }
}
