use std::sync::Arc;

use rikka_types::{Color, Point, Rect};
use rustc_hash::FxHashSet;

use crate::fx::PostEffect;
use crate::scene::damage::{DamagePolicy, ViewerPending};
use crate::scene::dirt::{BOUNDS, CONTENT, Dirt, NEW, ORDER, Reset, SHIFTED};
use crate::scene::node::{GroupClip, GroupId, LayerId, Node, NodeId};
use crate::scene::scene_impl::{Scene, texture_of};

type Placing = (Option<GroupId>, Point, Rect, GroupClip);

type LayerFade = (Option<LayerId>, f32);

const PUT_AHEAD: usize = 2;

#[derive(Default)]
pub struct SceneFrame {
    policy: DamagePolicy,
    reset: Option<Reset>,
    background: Option<Color>,
    effects: Option<Vec<Arc<dyn PostEffect>>>,
    groups: Vec<(GroupId, Option<Placing>)>,
    layers: Vec<(LayerId, Option<LayerFade>)>,
    removed: Vec<NodeId>,
    nodes: Vec<(NodeId, Dirt, Node)>,
    spent: Vec<(NodeId, Dirt, Node)>,
    damage: ViewerPending,
}

#[derive(Debug, Default)]
pub(crate) struct FrameTracking {
    seen_groups: FxHashSet<GroupId>,
    gone_groups: FxHashSet<GroupId>,
    seen_layers: FxHashSet<LayerId>,
    gone_layers: FxHashSet<LayerId>,
}

impl SceneFrame {
    pub fn is_empty(&self) -> bool {
        self.reset.is_none()
            && self.background.is_none()
            && self.effects.is_none()
            && self.groups.is_empty()
            && self.layers.is_empty()
            && self.removed.is_empty()
            && self.nodes.is_empty()
            && self.damage.is_empty()
    }

    pub fn nodes(&self) -> usize {
        self.nodes.len()
    }
}

impl std::fmt::Debug for SceneFrame {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SceneFrame")
            .field("reset", &self.reset)
            .field("nodes", &self.nodes.len())
            .field("removed", &self.removed.len())
            .field("groups", &self.groups.len())
            .field("layers", &self.layers.len())
            .finish_non_exhaustive()
    }
}

impl Scene {
    pub fn take_frame(&mut self, into: &mut SceneFrame) {
        into.groups.clear();
        into.layers.clear();
        into.nodes.clear();
        into.spent.clear();
        into.removed.clear();
        for (at, id) in self.journal.nodes.iter().enumerate() {
            if let Some(ahead) = self.journal.nodes.get(at + PUT_AHEAD) {
                self.nodes.prefetch(ahead.index);
            }
            let Some(node) = self.nodes.get_mut(id.index, id.generation) else {
                continue;
            };
            let dirt = std::mem::take(&mut node.dirt) & !SHIFTED;
            if dirt != 0 {
                into.nodes.push((*id, dirt, node.clone()));
            }
        }
        let FrameTracking {
            seen_groups,
            gone_groups,
            seen_layers,
            gone_layers,
        } = &mut self.tracking;
        seen_groups.clear();
        gone_groups.clear();
        gone_groups.extend(self.journal.gone_groups.iter().copied());
        for id in self.journal.groups.iter().chain(&self.journal.gone_groups) {
            if !seen_groups.insert(*id) {
                continue;
            }
            let state = match gone_groups.contains(id) {
                true => None,
                false => self
                    .groups
                    .get(id.index, id.generation)
                    .map(|group| (group.parent, group.offset, group.shown, group.clip)),
            };
            into.groups.push((*id, state));
        }
        seen_layers.clear();
        gone_layers.clear();
        gone_layers.extend(self.journal.gone_layers.iter().copied());
        for id in self.journal.layers.iter().chain(&self.journal.gone_layers) {
            if !seen_layers.insert(*id) {
                continue;
            }
            let state = match gone_layers.contains(id) {
                true => None,
                false => self
                    .layers
                    .get(id.index, id.generation)
                    .map(|layer| (layer.parent, layer.opacity)),
            };
            into.layers.push((*id, state));
        }
        std::mem::swap(&mut into.removed, &mut self.journal.removed);
        into.policy = self.damage.policy();
        into.reset = self.journal.reset;
        into.background = self.journal.background.then_some(self.background);
        into.effects = self.journal.effects.then(|| self.effects.clone());
        self.damage.transfer_into(&mut into.damage);
        self.journal.clear();
        self.clear_order();
    }

    pub fn take_whole_frame(&mut self, into: &mut SceneFrame) {
        let whole: Vec<NodeId> = self
            .nodes
            .iter()
            .map(|((index, generation), _)| NodeId { index, generation })
            .collect();
        for id in whole {
            self.mark(id, NEW | CONTENT | BOUNDS | ORDER);
        }
        let groups = self
            .groups
            .iter()
            .map(|((index, generation), _)| GroupId { index, generation });
        self.journal.groups.extend(groups);
        let layers = self
            .layers
            .iter()
            .map(|((index, generation), _)| LayerId { index, generation });
        self.journal.layers.extend(layers);
        self.journal.removed.clear();
        self.journal.gone_groups.clear();
        self.journal.gone_layers.clear();
        self.journal.reset = Some(Reset::Cleared);
        self.journal.background = true;
        self.journal.effects = true;
        self.damage.mark_full();
        self.take_frame(into);
    }

    pub fn apply(&mut self, frame: &mut SceneFrame) {
        self.damage.set_policy(frame.policy);
        if let Some(reset) = frame.reset.take() {
            self.forget_everything(reset);
        }
        if let Some(background) = frame.background.take() {
            self.set_background(background);
        }
        if let Some(effects) = frame.effects.take() {
            self.effects = effects;
            self.journal.effects = true;
            self.damage.mark_full();
        }
        for (id, state) in &frame.groups {
            if state.is_none() {
                self.remove_group(*id);
            }
        }
        for (id, state) in &frame.groups {
            if state.is_some() {
                self.put_group(*id);
            }
        }
        for (id, state) in frame.groups.drain(..) {
            if let Some((parent, offset, shown, clip)) = state {
                self.set_group(id, parent, offset, shown);
                self.set_group_clip(id, clip);
            }
        }
        for (id, state) in &frame.layers {
            if state.is_none() {
                self.remove_layer(*id);
            }
        }
        for (id, state) in &frame.layers {
            if state.is_some() {
                self.put_layer(*id);
            }
        }
        for (id, state) in frame.layers.drain(..) {
            if let Some((parent, opacity)) = state {
                self.set_layer(id, parent, opacity);
            }
        }
        for id in frame.removed.drain(..) {
            self.remove(id);
        }
        for at in 0..frame.nodes.len() {
            if let Some((ahead, _, _)) = frame.nodes.get(at + PUT_AHEAD) {
                self.nodes.prefetch(ahead.index);
            }
            let (id, dirt, node) = &mut frame.nodes[at];
            self.put(*id, *dirt, node);
        }
        frame.spent.clear();
        std::mem::swap(&mut frame.nodes, &mut frame.spent);
        self.damage.receive(&mut frame.damage);
        self.settle_order();
    }

    fn put(&mut self, id: NodeId, dirt: Dirt, node: &mut Node) {
        node.dirt = 0;
        let replaced = self
            .nodes
            .swap(id.index, id.generation, node, || Node::rect(Rect::ZERO));
        let old = node;
        let fresh = match replaced {
            Some(generation) if generation == id.generation => {
                let (old_texture, had_backdrop, was_unclipped) = (
                    texture_of(old),
                    old.backdrop.is_some(),
                    old.escapes_group_clip(),
                );
                let (new_texture, has_backdrop, is_unclipped, group, clip_group) = {
                    let now = self
                        .nodes
                        .get_mut(id.index, id.generation)
                        .expect("entry exists");
                    now.dirt = old.dirt;
                    (
                        texture_of(now),
                        now.backdrop.is_some(),
                        now.escapes_group_clip(),
                        now.group,
                        now.clip_group,
                    )
                };
                if old_texture != new_texture {
                    if let Some(old_tex) = old_texture {
                        self.forget_texture_use(old_tex, id);
                    }
                    if let Some(new_tex) = new_texture {
                        self.add_texture_use(new_tex, id);
                    }
                }
                if had_backdrop != has_backdrop {
                    self.count_backdrop(id, had_backdrop, has_backdrop);
                }
                if was_unclipped != is_unclipped {
                    self.count_unclipped(id, was_unclipped, is_unclipped);
                }
                if clip_group != old.clip_group {
                    self.move_clip(id, group, old.clip_group, clip_group);
                }
                false
            }
            Some(generation) => {
                let gone = NodeId {
                    index: id.index,
                    generation,
                };
                self.untrack(gone, old);
                self.remove_from_groups(old);
                self.journal.removed.push(gone);
                if old.layer.is_some() {
                    self.order_dirty = true;
                }
                self.order_pruned = true;
                true
            }
            None => true,
        };
        let dirt = match fresh {
            true => dirt | NEW | CONTENT | BOUNDS | ORDER,
            false => dirt,
        };
        self.track_node(id, dirt, fresh);
    }
}
