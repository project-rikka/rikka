use std::collections::{HashMap, HashSet};

use paint_document::{Edit, Layer, LayerId, LayerKind, Placement};
use rikka_app::prelude::*;

use super::Tab;
use crate::wording;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Layering {
    New,
    Duplicate,
    Delete,
    Forward,
    Backward,
    MergeDown,
    MergeVisible,
    Flatten,
    FromBackground,
}

#[derive(Debug, Default)]
pub(crate) struct Arranging {
    aim: Option<Placement>,
    menu: Option<(LayerId, Point)>,
}

impl Tab {
    pub(crate) fn aim(&self) -> Option<Placement> {
        self.arranging.aim
    }

    pub(crate) fn layer_menu(&self) -> Option<(LayerId, Point)> {
        self.arranging.menu
    }

    pub(crate) fn layering(&self, order: Layering) -> Option<Edit> {
        if !self.edits() {
            return None;
        }
        let active_layer = self.active_layer();
        let edit = match order {
            Layering::New => Edit::NewLayer {
                above: active_layer.or(self.layers.first())?.id,
                name: numbered(&self.layers, 1),
            },
            Layering::Duplicate => Edit::DuplicateLayer {
                layer: active_layer?.id,
                name: tr!(
                    "paint.layers.copy_of",
                    name = &wording::layer_name(active_layer?).to_string()
                )
                .to_string(),
            },
            Layering::Delete => Edit::DeleteLayer {
                layer: active_layer?.id,
            },
            Layering::Forward => Edit::MoveLayer {
                layer: active_layer?.id,
                to: Placement::Forward,
            },
            Layering::Backward => Edit::MoveLayer {
                layer: active_layer?.id,
                to: Placement::Backward,
            },
            Layering::MergeDown => Edit::MergeDown {
                layer: active_layer?.id,
            },
            Layering::MergeVisible => Edit::MergeVisible,
            Layering::Flatten => Edit::Flatten,
            Layering::FromBackground => Edit::LayerFromBackground {
                name: numbered(&self.layers, 0),
            },
        };
        edit.check(&self.layers).is_ok().then_some(edit)
    }

    pub(crate) fn arrange(&mut self, order: Layering) {
        if let Some(edit) = self.layering(order) {
            self.order(edit);
        }
    }

    pub(crate) fn restyles(&self) -> bool {
        self.edits() && self.active_layer().is_some_and(|layer| !layer.background)
    }

    pub(crate) fn aim_at(&mut self, aim: Option<Placement>) -> bool {
        let changed = self.arranging.aim != aim;
        self.arranging.aim = aim;
        changed
    }

    pub(crate) fn drop_layer(&mut self, layer: LayerId, to: Placement) {
        self.arranging.aim = None;
        let edit = Edit::MoveLayer { layer, to };
        if self.edits() && edit.check(&self.layers).is_ok() {
            self.order(edit);
        }
    }

    pub(crate) fn open_layer_menu(&mut self, layer: LayerId, at: Point) {
        self.activate(layer);
        self.arranging.menu = Some((layer, at));
    }

    pub(crate) fn close_layer_menu(&mut self) {
        self.arranging.menu = None;
    }

    pub(super) fn regroup(&mut self, layers: &[Layer]) {
        let mut paths = HashMap::new();
        named_paths(&self.layers, &mut Vec::new(), &mut |id, path| {
            paths.insert(id, path.to_vec());
        });
        let closed: HashSet<Vec<(String, usize)>> = self
            .closed
            .iter()
            .filter_map(|id| paths.remove(id))
            .collect();
        let mut regrouped = HashSet::new();
        named_paths(layers, &mut Vec::new(), &mut |id, path| {
            if closed.contains(path) {
                regrouped.insert(id);
            }
        });
        self.closed = regrouped;
    }
}

fn numbered(layers: &[Layer], from: u32) -> String {
    let mut used_names = HashSet::new();
    every_name(layers, &mut used_names);
    let last = from + used_names.len() as u32;
    let name = |number: u32| tr!("paint.layers.numbered", number = number).to_string();
    (from..last)
        .map(name)
        .find(|candidate| !used_names.contains(candidate))
        .unwrap_or_else(|| name(last))
}

fn every_name(layers: &[Layer], names: &mut HashSet<String>) {
    for layer in layers {
        names.insert(layer.name.clone());
        every_name(&layer.children, names);
    }
}

fn named_paths(
    layers: &[Layer],
    path: &mut Vec<(String, usize)>,
    visit: &mut impl FnMut(LayerId, &[(String, usize)]),
) {
    let mut name_counts: HashMap<&str, usize> = HashMap::new();
    for layer in layers {
        let count = name_counts.entry(layer.name.as_str()).or_default();
        path.push((layer.name.clone(), *count));
        *count += 1;
        if layer.kind == LayerKind::Group {
            visit(layer.id, path);
            named_paths(&layer.children, path, visit);
        }
        path.pop();
    }
}
