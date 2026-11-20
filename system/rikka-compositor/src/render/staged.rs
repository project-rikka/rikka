use std::collections::HashMap;

use rikka_compositor_desktop::{SnapshotId, WindowId};
use rikka_compositor_geometry::{Logical, Physical, Rect};
use rikka_renderer::Scene;
use rikka_types::{Color, RoundedRect};

use super::depth::Depth;
use super::frozen::{Frozen, FrozenNodes};
use super::panels::PanelNodes;
use super::staging::{Leaving, Staging};
use super::textures::Look;
use super::tree::SurfaceTree;
use super::view::View;
use crate::glass::{Frost, frost};

const TOP_Z: i32 = i32::MAX - 4096;

#[derive(Debug, Default)]
pub(crate) struct Staged {
    snapshots: HashMap<SnapshotId, Frozen>,
    frozen: FrozenNodes,
    beneath: PanelNodes,
    over: PanelNodes,
    top: PanelNodes,
    view: Option<View>,
}

impl Staged {
    pub(crate) fn mark_seen(&mut self, view: View) {
        self.view = Some(view);
    }

    pub(crate) fn freeze(
        &mut self,
        tree: &SurfaceTree,
        snapshot: SnapshotId,
        geometry: Rect<i32, Logical>,
        frosted: &[RoundedRect],
    ) -> Option<Vec<Look>> {
        let frozen = Frozen::capture(tree, self.view?, geometry, frost(frosted))?;
        let looks = frozen.looks().collect();
        self.snapshots.insert(snapshot, frozen);
        Some(looks)
    }

    pub(crate) fn thaw(&mut self, snapshot: SnapshotId) -> Vec<Look> {
        self.snapshots
            .remove(&snapshot)
            .map(|frozen| frozen.looks().collect())
            .unwrap_or_default()
    }

    pub(crate) fn thaw_all(&mut self) -> Vec<Look> {
        self.snapshots
            .drain()
            .flat_map(|(_, frozen)| frozen.looks().collect::<Vec<_>>())
            .collect()
    }

    pub(crate) fn beneath(
        &mut self,
        scene: &mut Scene,
        staging: &Staging,
        window: WindowId,
        depth: &mut Depth,
    ) -> bool {
        if let Some((under, panel)) = &staging.beneath
            && *under == window
        {
            self.beneath.show(scene, Some(panel), depth);
            return true;
        }
        false
    }

    pub(crate) fn leaving(
        &mut self,
        scene: &mut Scene,
        leaving: &Leaving,
        view: View,
        edge: Color,
        depth: &mut Depth,
    ) {
        if let Some(frozen_snapshot) = self.snapshots.get(&leaving.snapshot) {
            self.frozen
                .draw(scene, frozen_snapshot, leaving, view, edge, depth);
        }
    }

    pub(crate) fn draw_into(&mut self, scene: &mut Scene, staging: &Staging, depth: &mut Depth) {
        self.over.show(scene, staging.over.as_ref(), depth);
    }

    pub(crate) fn frost(&self, snapshot: SnapshotId) -> Option<&Frost> {
        self.snapshots.get(&snapshot).map(Frozen::frost)
    }

    pub(crate) fn finish(&mut self, scene: &mut Scene, staging: &Staging) {
        if staging.beneath.is_none() {
            self.beneath.hide(scene);
        }
        if staging.over.is_none() {
            self.over.hide(scene);
        }
        let mut depth = Depth::above(TOP_Z);
        self.top.show(scene, staging.top.as_ref(), &mut depth);
        self.frozen.sweep(scene);
    }

    pub(crate) fn beneath_drawn(
        &self,
    ) -> impl Iterator<Item = (usize, Rect<i32, Physical>, f32)> + '_ {
        self.beneath.drawn()
    }

    pub(crate) fn over_drawn(
        &self,
    ) -> impl Iterator<Item = (usize, Rect<i32, Physical>, f32)> + '_ {
        self.over.drawn()
    }

    pub(crate) fn frozen_drawn_for(
        &self,
        snapshot: SnapshotId,
    ) -> impl Iterator<Item = (usize, Rect<i32, Physical>, f32, Option<RoundedRect>)> + '_ {
        self.frozen.drawn_for(snapshot)
    }
}
