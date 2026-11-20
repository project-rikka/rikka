use std::collections::{BTreeMap, BTreeSet};

use crate::context::{Site, Taint};
use crate::error::{GraphError, Problem};
use crate::flat::{Flat, FlatNode, Path, Source};
use crate::library::{Behavior, ClosureCode};
use crate::node::NodeKind;

pub(crate) enum Closure<'a> {
    Custom,
    Model(&'a ClosureCode),
}

pub(crate) fn closure<'a>(flat: &Flat<'a>, node: &FlatNode<'_>) -> Option<Closure<'a>> {
    match node.kind {
        NodeKind::Lighting => Some(Closure::Custom),
        NodeKind::Extern(_) => match &flat.spec(node)?.behavior {
            Behavior::Closure(code) => Some(Closure::Model(code)),
            _ => None,
        },
        _ => None,
    }
}

fn shades(flat: &Flat<'_>, node: &FlatNode<'_>) -> bool {
    closure(flat, node).is_some()
        || matches!(
            node.kind,
            NodeKind::Emission
                | NodeKind::Transparent
                | NodeKind::MixShader
                | NodeKind::AddShader
                | NodeKind::ShaderToRgb
        )
}

pub(crate) fn ignored(node: &FlatNode<'_>) -> Option<&'static str> {
    match node.kind {
        NodeKind::ColorRamp(ramp) if ramp.stops.len() < 2 => Some("position"),
        NodeKind::Curve(curve) if curve.points.len() < 2 => Some("value"),
        _ => None,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Phase {
    Level(usize),
    Light(usize),
    Indirect(usize),
    Closure(usize),
}

pub(crate) struct Side {
    pub hull: bool,
    pub roots: Vec<(&'static str, Source)>,
    pub order: Vec<Path>,
    pub taints: BTreeMap<Path, Taint>,
    pub levels: BTreeMap<Path, usize>,
    pub stages: BTreeMap<Path, usize>,
    pub uses: BTreeMap<Path, BTreeSet<usize>>,
    pub loops: usize,
}

impl Side {
    pub(crate) fn level(&self, source: &Source) -> usize {
        match source {
            Source::Link(from, _) => self.levels.get(from).copied().unwrap_or(0),
            _ => 0,
        }
    }

    pub(crate) fn phase(&self, path: &Path) -> Phase {
        if let Some(stage) = self.stages.get(path) {
            return Phase::Closure(*stage);
        }
        let first = || {
            self.uses
                .get(path)
                .and_then(|uses| uses.first().copied())
                .unwrap_or(0)
        };
        match self.taints.get(path) {
            Some(Taint::Light) => Phase::Light(first()),
            Some(Taint::Indirect) => Phase::Indirect(first()),
            None => Phase::Level(self.levels.get(path).copied().unwrap_or(0)),
        }
    }
}

fn tainted(taint: Taint) -> Problem {
    match taint {
        Taint::Light => Problem::PerLight,
        Taint::Indirect => Problem::PerAmbient,
    }
}

pub(crate) fn stage(
    flat: &Flat<'_>,
    output: &FlatNode<'_>,
    hull: bool,
    ports: &[&'static str],
    errors: &mut Vec<GraphError>,
) -> Side {
    let roots: Vec<(&'static str, Source)> = ports
        .iter()
        .map(|port| (*port, output.input(port).clone()))
        .collect();
    let cone = flat.cone(roots.iter().filter_map(|(_, source)| match source {
        Source::Link(from, _) => Some(from.clone()),
        _ => None,
    }));
    let order = flat.order(&cone);
    let mut side = Side {
        hull,
        roots,
        order,
        taints: BTreeMap::new(),
        levels: BTreeMap::new(),
        stages: BTreeMap::new(),
        uses: BTreeMap::new(),
        loops: 0,
    };
    for path in &side.order {
        let node = &flat.nodes[path];
        let mut taint = flat.taint(node);
        let mut level = 0;
        let sink = shades(flat, node);
        let skipped = ignored(node);
        for (port, from, _) in node.sources() {
            if skipped == Some(port) {
                continue;
            }
            level = level.max(side.levels.get(from).copied().unwrap_or(0));
            let Some(found) = side.taints.get(from).copied() else {
                continue;
            };
            let accepted = match (node.kind, port) {
                (NodeKind::Lighting, "light") => found == Taint::Light,
                (NodeKind::Lighting, "ambient") => found == Taint::Indirect,
                _ => !sink,
            };
            if !accepted {
                errors.push(GraphError::new(node.at(Some(port)), tainted(found)));
                continue;
            }
            if sink {
                continue;
            }
            match taint {
                Some(known) if known != found => {
                    errors.push(GraphError::new(node.at(Some(port)), Problem::MixedLighting));
                }
                _ => taint = Some(found),
            }
        }
        if closure(flat, node).is_some() {
            side.stages.insert(path.clone(), level);
            level += 1;
        }
        side.levels.insert(path.clone(), level);
        if let Some(taint) = taint {
            side.taints.insert(path.clone(), taint);
        }
    }
    for (port, source) in &side.roots {
        if let Source::Link(from, _) = source {
            if let Some(found) = side.taints.get(from) {
                errors.push(GraphError::new(output.at(Some(port)), tainted(*found)));
            }
            side.loops = side.loops.max(side.levels[from]);
        }
    }
    let closures: Vec<(Path, usize)> = side
        .stages
        .iter()
        .map(|(path, stage)| (path.clone(), *stage))
        .collect();
    let read = |path: &Path| -> Vec<Path> {
        let node = &flat.nodes[path];
        let skipped = ignored(node);
        node.sources()
            .filter(|(port, _, _)| skipped != Some(*port))
            .map(|(_, from, _)| from.clone())
            .collect()
    };
    for (path, stage) in closures {
        let mut pending = read(&path);
        while let Some(from) = pending.pop() {
            if !side.taints.contains_key(&from) {
                continue;
            }
            if side.uses.entry(from.clone()).or_default().insert(stage) {
                pending.extend(read(&from));
            }
        }
    }
    let phase = |wanted: Option<Taint>, post: bool| -> BTreeSet<Path> {
        side.order
            .iter()
            .filter(|path| {
                side.taints.get(*path).copied() == wanted
                    && (wanted.is_some() || (side.levels[*path] > 0) == post)
            })
            .cloned()
            .collect()
    };
    let surface: BTreeSet<Path> = phase(None, false)
        .into_iter()
        .chain(
            side.stages
                .iter()
                .filter(|(_, stage)| **stage == 0)
                .map(|(path, _)| path.clone()),
        )
        .collect();
    let post: BTreeSet<Path> = phase(None, true)
        .into_iter()
        .filter(|path| side.stages.get(path) != Some(&0))
        .collect();
    errors.extend(flat.site_errors(&surface, Site::Surface));
    errors.extend(flat.site_errors(&post, Site::Post));
    errors.extend(flat.site_errors(&phase(Some(Taint::Light), false), Site::Light));
    errors.extend(flat.site_errors(&phase(Some(Taint::Indirect), false), Site::Indirect));
    side
}
