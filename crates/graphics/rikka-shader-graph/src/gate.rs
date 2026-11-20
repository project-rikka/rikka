use std::collections::{BTreeMap, BTreeSet};

use crate::context::{Site, input_code};
use crate::emit::{Emitter, Value, coerce, text, zero};
use crate::flat::{Flat, FlatNode, Path, Slot, Source};
use crate::graph::SocketRef;
use crate::library::{Behavior, Bypass, Field};
use crate::node::{BlendMode, Input, NodeKind};
use crate::ops::{Needs, Op};
use crate::staging::{Closure, Phase, Side, closure};
use crate::types::{Constant, SocketValue, ValueType};

const GATED_WEIGHT: usize = 4;

const VARYING_WEIGHT: usize = 64;

const CLOSURE_WEIGHT: usize = 64;

const FEATURE_WEIGHT: usize = 1;

const FIRST_FEATURE: u32 = 7;

const LAST_FEATURE: u32 = 30;

const VARYING_TOKENS: [&str; 7] = [
    "$pixel",
    "$light",
    "$shadow",
    "$indirect",
    "$vertex",
    "$uv",
    "$texel",
];

pub(crate) type GateId = usize;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Test {
    Equal(f32),
    Above(f32),
    NotAbove(f32),
    AtLeast(f32),
    AtMost(f32),
}

impl Test {
    fn skips(self, constant: &Constant) -> bool {
        let first = constant.component(0);
        match self {
            Self::Equal(value) => constant.values().iter().all(|found| *found == value),
            Self::Above(value) => first > value,
            Self::NotAbove(value) => first.partial_cmp(&value) != Some(std::cmp::Ordering::Greater),
            Self::AtLeast(value) => first >= value,
            Self::AtMost(value) => first <= value,
        }
    }

    fn runs(self, value: &str, width: usize) -> String {
        let float = |number: f32| crate::parameter::float(number);
        match self {
            Self::Equal(number) if width > 1 => format!("any({value} != {})", float(number)),
            Self::Equal(number) => format!("{value} != {}", float(number)),
            Self::Above(number) => format!("!({value} > {})", float(number)),
            Self::NotAbove(number) => format!("{value} > {}", float(number)),
            Self::AtLeast(number) => format!("!({value} >= {})", float(number)),
            Self::AtMost(number) => format!("!({value} <= {})", float(number)),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
enum Effect {
    Operand(&'static str),
    Node(Bypass),
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Control {
    Socket(SocketRef),
    Parameter(String),
}

#[derive(Debug, Default)]
pub(crate) struct Features {
    bits: BTreeMap<Control, u32>,
}

impl Features {
    fn bit(&mut self, control: &Control) -> Option<u32> {
        if let Some(bit) = self.bits.get(control) {
            return Some(*bit);
        }
        let next = FIRST_FEATURE + u32::try_from(self.bits.len()).ok()?;
        (next <= LAST_FEATURE).then(|| {
            self.bits.insert(control.clone(), next);
            next
        })
    }

    pub(crate) fn controls(&self) -> impl Iterator<Item = (&Control, u32)> {
        self.bits.iter().map(|(control, bit)| (control, *bit))
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.bits.is_empty()
    }
}

#[derive(Clone, Debug)]
struct Branch {
    node: Path,
    control: String,
    test: Test,
    effect: Effect,
    uniform: bool,
    fixed: bool,
    key: Option<Control>,
    feature: Option<u32>,
}

impl Branch {
    fn keeps(&self, port: &str) -> bool {
        match &self.effect {
            Effect::Operand(_) => true,
            Effect::Node(Bypass::Input(input)) => port == self.control || port == *input,
            Effect::Node(Bypass::Constant(_)) => port == self.control,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Condition {
    Never,
    Always,
    When(String),
}

pub(crate) enum Item {
    Node(Path),
    Block(GateId, Vec<Item>),
}

pub(crate) trait Imports {
    fn import(&mut self, emitter: &mut Emitter<'_, '_>, node: &FlatNode<'_>);
    fn import_port(&mut self, emitter: &mut Emitter<'_, '_>, node: &FlatNode<'_>, port: &str);
}

pub(crate) struct Gating<'f, 'g> {
    flat: &'f Flat<'g>,
    branches: Vec<Branch>,
    guards: BTreeMap<Path, BTreeSet<GateId>>,
    runs: BTreeMap<Path, BTreeSet<GateId>>,
    depth: Vec<usize>,
    users: BTreeMap<Slot, Vec<Path>>,
}

struct Uniformity<'f, 'g> {
    flat: &'f Flat<'g>,
    known: BTreeMap<Path, bool>,
}

impl Uniformity<'_, '_> {
    fn source(&mut self, source: &Source) -> bool {
        match source {
            Source::Constant(_) | Source::Socket(..) | Source::Empty => true,
            Source::Context(input) => *input == Input::Time,
            Source::Link(from, _) => self.node(from),
            Source::Links(links) => links.iter().all(|(from, _)| self.node(from)),
            Source::Missing => false,
        }
    }

    fn node(&mut self, path: &Path) -> bool {
        if let Some(known) = self.known.get(path) {
            return *known;
        }
        let flat = self.flat;
        let Some(node) = flat.nodes.get(path) else {
            return false;
        };
        let own = match node.kind {
            NodeKind::Constant(_) | NodeKind::Parameter(_) => true,
            NodeKind::Input(input) => *input == Input::Time,
            NodeKind::Op(op) => op.info().needs == Needs::Nothing,
            NodeKind::Swizzle(_)
            | NodeKind::FunctionInput(_)
            | NodeKind::FunctionOutput(_)
            | NodeKind::Blend(_)
            | NodeKind::ColorRamp(_)
            | NodeKind::Curve(_)
            | NodeKind::Fbm(_)
            | NodeKind::Texture(_) => true,
            NodeKind::Extern(_) => match flat.spec(node).map(|spec| &spec.behavior) {
                Some(Behavior::Function(function)) => VARYING_TOKENS
                    .iter()
                    .all(|token| !function.code.contains(token)),
                _ => false,
            },
            _ => false,
        };
        let uniform = own && node.inputs.iter().all(|(_, source)| self.source(source));
        self.known.insert(path.clone(), uniform);
        uniform
    }
}

fn differentiates(flat: &Flat<'_>, node: &FlatNode<'_>) -> bool {
    match node.kind {
        NodeKind::Texture(_) => true,
        NodeKind::Op(op) => op.info().needs == Needs::Pixel,
        NodeKind::Extern(_) => match flat.spec(node).map(|spec| &spec.behavior) {
            Some(Behavior::Function(function)) => function.code.contains("$pixel"),
            Some(Behavior::Closure(_)) => true,
            _ => false,
        },
        _ => false,
    }
}

fn weight(flat: &Flat<'_>, node: &FlatNode<'_>) -> usize {
    match node.kind {
        NodeKind::Op(
            Op::ValueNoise
            | Op::GradientNoise
            | Op::Voronoi
            | Op::WhiteNoise
            | Op::Brick
            | Op::Wave
            | Op::Parallax
            | Op::SkyIrradiance
            | Op::SkyReflection
            | Op::IndirectDiffuse,
        ) => 4,
        NodeKind::Op(_) | NodeKind::ColorRamp(_) | NodeKind::Curve(_) => 1,
        NodeKind::Blend(_) => 2,
        NodeKind::Texture(_) => 2,
        NodeKind::Fbm(_) => 8,
        NodeKind::Lighting => 1,
        NodeKind::Extern(_) => match flat.spec(node).map(|spec| &spec.behavior) {
            Some(Behavior::Closure(_)) => CLOSURE_WEIGHT,
            _ => 4,
        },
        _ => 0,
    }
}

fn candidates(
    flat: &Flat<'_>,
    node: &FlatNode<'_>,
    uniformity: &mut Uniformity<'_, '_>,
) -> Vec<(&'static str, Test, Effect)> {
    match node.kind {
        NodeKind::Op(Op::Lerp) => vec![
            ("t", Test::Equal(1.0), Effect::Operand("a")),
            ("t", Test::Equal(0.0), Effect::Operand("b")),
        ],
        NodeKind::Op(Op::Multiply) => [("b", "a"), ("a", "b")]
            .into_iter()
            .filter(|(weight, _)| uniformity.source(node.input(weight)))
            .map(|(weight, other)| (weight, Test::Equal(0.0), Effect::Operand(other)))
            .collect(),
        NodeKind::Op(Op::Switch) => vec![
            ("condition", Test::Above(0.5), Effect::Operand("a")),
            ("condition", Test::NotAbove(0.5), Effect::Operand("b")),
        ],
        NodeKind::Blend(mode) => {
            let mut found = vec![(
                "opacity",
                Test::Equal(0.0),
                Effect::Node(Bypass::Input("base")),
            )];
            if *mode == BlendMode::Normal {
                found.push(("opacity", Test::Equal(1.0), Effect::Operand("base")));
            }
            found
        }
        NodeKind::MixShader => vec![
            ("factor", Test::AtLeast(1.0), Effect::Operand("a")),
            ("factor", Test::AtMost(0.0), Effect::Operand("b")),
        ],
        NodeKind::Extern(_) => {
            let Some(spec) = flat.spec(node) else {
                return Vec::new();
            };
            let Behavior::Function(function) = &spec.behavior else {
                return Vec::new();
            };
            let Some(gate) = function.gate else {
                return Vec::new();
            };
            let declared = |port: &str| spec.inputs.iter().any(|input| input.name == port);
            let bypassed = match gate.bypass {
                Bypass::Input(input) => declared(input),
                Bypass::Constant(_) => true,
            };
            match spec.outputs.len() == 1 && declared(gate.input) && bypassed {
                true => vec![(gate.input, Test::Equal(0.0), Effect::Node(gate.bypass))],
                false => Vec::new(),
            }
        }
        _ => Vec::new(),
    }
}

fn control(flat: &Flat<'_>, source: &Source) -> Option<Control> {
    match source {
        Source::Socket(at, SocketValue::Number(_)) => Some(Control::Socket(at.clone())),
        Source::Link(from, _) => match flat.nodes.get(from)?.kind {
            NodeKind::Parameter(name)
                if flat
                    .graph
                    .parameter(name)
                    .is_some_and(|parameter| parameter.ty() != ValueType::Texture) =>
            {
                Some(Control::Parameter(name.clone()))
            }
            _ => None,
        },
        _ => None,
    }
}

fn sites(phase: Phase) -> &'static [Site] {
    match phase {
        Phase::Level(0) => &[Site::Surface],
        Phase::Level(_) => &[Site::Post],
        Phase::Light(_) => &[Site::Light],
        Phase::Indirect(_) => &[Site::Indirect],
        Phase::Closure(0) => &[Site::Surface, Site::Post, Site::Light, Site::Indirect],
        Phase::Closure(_) => &[Site::Post, Site::Light, Site::Indirect],
    }
}

fn slot_name(slot: &Slot, depth: usize) -> String {
    let path: Vec<String> = slot.0.iter().map(ToString::to_string).collect();
    let port: String = slot
        .1
        .chars()
        .map(|letter| match letter.is_ascii_alphanumeric() {
            true => letter.to_ascii_lowercase(),
            false => '_',
        })
        .collect();
    format!("gated{depth}_{}_{port}", path.join("_"))
}

struct Guards {
    guards: BTreeMap<Path, BTreeSet<GateId>>,
    runs: BTreeMap<Path, BTreeSet<GateId>>,
}

impl<'f, 'g> Gating<'f, 'g> {
    pub(crate) fn new(
        flat: &'f Flat<'g>,
        side: &Side,
        main: Option<&Path>,
        features: &mut Features,
    ) -> Self {
        let mut uniformity = Uniformity {
            flat,
            known: BTreeMap::new(),
        };
        let mut branches = Vec::new();
        for path in &side.order {
            let node = &flat.nodes[path];
            for (control, test, effect) in candidates(flat, node, &mut uniformity) {
                let source = node.input(control);
                let fixed = match source {
                    Source::Constant(constant) if test.skips(constant) => true,
                    Source::Constant(_) | Source::Missing | Source::Links(_) => continue,
                    _ => false,
                };
                let uniform = uniformity.source(source);
                let key = match test {
                    Test::Equal(0.0) => self::control(flat, source),
                    _ => None,
                };
                branches.push(Branch {
                    node: path.clone(),
                    control: control.to_owned(),
                    test,
                    effect,
                    uniform,
                    fixed,
                    key,
                    feature: None,
                });
            }
        }
        let mut users: BTreeMap<Slot, Vec<Path>> = BTreeMap::new();
        let output = flat.output().map(|(path, _)| path.clone());
        for path in side.order.iter().chain(output.as_ref()) {
            for (_, from, port) in flat.nodes[path].sources() {
                users
                    .entry((from.clone(), port.to_owned()))
                    .or_default()
                    .push(path.clone());
            }
        }
        let mut gating = Self {
            flat,
            branches,
            guards: BTreeMap::new(),
            runs: BTreeMap::new(),
            depth: Vec::new(),
            users,
        };
        let all = vec![true; gating.branches.len()];
        let first = gating.compute(side, main, &all);
        let mut weights = vec![0; gating.branches.len()];
        for (path, set) in &first.guards {
            let cost = weight(flat, &flat.nodes[path]);
            for gate in set {
                weights[*gate] += cost;
            }
        }
        for set in first.runs.values() {
            for gate in set {
                weights[*gate] += CLOSURE_WEIGHT;
            }
        }
        let runtime = |branch: &Branch| match branch.uniform {
            true => GATED_WEIGHT,
            false => VARYING_WEIGHT,
        };
        let mut enabled = Vec::with_capacity(gating.branches.len());
        for (branch, weight) in gating.branches.iter_mut().zip(&weights) {
            if branch.fixed {
                enabled.push(true);
                continue;
            }
            if *weight >= FEATURE_WEIGHT
                && let Some(key) = &branch.key
            {
                branch.feature = features.bit(key);
            }
            enabled.push(branch.feature.is_some() || *weight >= runtime(branch));
        }
        let computed = gating.compute(side, main, &enabled);
        gating.guards = computed.guards;
        gating.runs = computed.runs;
        gating.depth = gating
            .branches
            .iter()
            .map(|branch| {
                gating.guards.get(&branch.node).map_or(0, |set| {
                    set.iter()
                        .filter(|gate| gating.branches[**gate].node != branch.node)
                        .count()
                })
            })
            .collect();
        gating
    }

    fn pinned(&self, side: &Side, main: Option<&Path>, path: &Path, port: &str) -> bool {
        let node = &self.flat.nodes[path];
        let Some(Closure::Model(model)) = closure(self.flat, node) else {
            return false;
        };
        if model.subsurface.is_some() && !side.hull && side.stages.get(path) == Some(&0) {
            return true;
        }
        main == Some(path)
            && model.fields.iter().any(|(name, field)| {
                *name == port && matches!(field, Field::Albedo | Field::Normal | Field::Occlusion)
            })
    }

    fn available(&self, side: &Side, source: &Source, phase: Phase) -> bool {
        match source {
            Source::Constant(_) | Source::Socket(..) | Source::Empty => true,
            Source::Context(input) => sites(phase)
                .iter()
                .all(|site| input_code(*input, *site).is_some()),
            Source::Link(from, _) => match (side.phase(from), phase) {
                (
                    Phase::Level(at),
                    Phase::Level(wanted)
                    | Phase::Light(wanted)
                    | Phase::Indirect(wanted)
                    | Phase::Closure(wanted),
                ) => at <= wanted,
                (Phase::Light(_), Phase::Light(_)) | (Phase::Indirect(_), Phase::Indirect(_)) => {
                    true
                }
                _ => false,
            },
            Source::Links(_) | Source::Missing => false,
        }
    }

    fn compute(&self, side: &Side, main: Option<&Path>, enabled: &[bool]) -> Guards {
        let flat = self.flat;
        let inside: BTreeSet<&Path> = side.order.iter().collect();
        let mut uses: BTreeMap<&Path, Vec<(&Path, &str)>> = BTreeMap::new();
        for path in &side.order {
            for (port, from, _) in flat.nodes[path].sources() {
                if inside.contains(from) {
                    uses.entry(from).or_default().push((path, port));
                }
            }
        }
        let rooted: BTreeSet<&Path> = side
            .roots
            .iter()
            .filter_map(|(_, source)| match source {
                Source::Link(from, _) => Some(from),
                _ => None,
            })
            .collect();
        let mut owned: BTreeMap<&Path, Vec<GateId>> = BTreeMap::new();
        for (gate, branch) in self.branches.iter().enumerate() {
            owned.entry(&branch.node).or_default().push(gate);
        }
        let mut guards: BTreeMap<Path, BTreeSet<GateId>> = BTreeMap::new();
        let mut runs: BTreeMap<Path, BTreeSet<GateId>> = BTreeMap::new();
        for path in side.order.iter().rev() {
            let node = &flat.nodes[path];
            let mut found: Option<BTreeSet<GateId>> = rooted.contains(path).then(BTreeSet::new);
            for (user, port) in uses.get(path).into_iter().flatten() {
                let guard = match self.pinned(side, main, user, port) {
                    true => BTreeSet::new(),
                    false => {
                        let mut guard: BTreeSet<GateId> = guards[*user]
                            .iter()
                            .copied()
                            .filter(|gate| {
                                let branch = &self.branches[*gate];
                                branch.node != **user
                                    || matches!(branch.effect, Effect::Node(_))
                                        && !branch.keeps(port)
                            })
                            .collect();
                        for gate in owned.get(user).into_iter().flatten() {
                            let branch = &self.branches[*gate];
                            if matches!(branch.effect, Effect::Operand(skipped) if skipped == *port)
                            {
                                guard.insert(*gate);
                            }
                        }
                        guard
                    }
                };
                found = Some(match found {
                    None => guard,
                    Some(known) => known.intersection(&guard).copied().collect(),
                });
            }
            let mut base = found.unwrap_or_default();
            let phase = side.phase(path);
            let keep = |set: &BTreeSet<GateId>, derivatives: bool| -> BTreeSet<GateId> {
                let mut viable: Vec<(usize, GateId, BTreeSet<GateId>)> = set
                    .iter()
                    .copied()
                    .filter(|gate| {
                        let branch = &self.branches[*gate];
                        let control = flat.nodes[&branch.node].input(&branch.control);
                        branch.fixed
                            || enabled[*gate]
                                && self.available(side, control, phase)
                                && !(derivatives && !branch.uniform && differentiates(flat, node))
                    })
                    .map(|gate| {
                        let owner = &self.branches[gate].node;
                        let enclosing: BTreeSet<GateId> = match owner == path {
                            true => BTreeSet::new(),
                            false => guards
                                .get(owner)
                                .into_iter()
                                .flatten()
                                .copied()
                                .filter(|outer| self.branches[*outer].node != *owner)
                                .collect(),
                        };
                        let rank = match owner == path {
                            true => usize::MAX,
                            false => enclosing.len(),
                        };
                        (rank, gate, enclosing)
                    })
                    .collect();
                viable.sort_by_key(|(rank, gate, _)| (*rank, *gate));
                let mut kept = BTreeSet::new();
                for (_, gate, enclosing) in viable {
                    if enclosing.is_subset(&kept) {
                        kept.insert(gate);
                    }
                }
                kept
            };
            let (guard, run) = match closure(flat, node) {
                Some(_) => (keep(&base, true), Some(keep(&base, false))),
                None => {
                    for gate in owned.get(path).into_iter().flatten() {
                        if matches!(self.branches[*gate].effect, Effect::Node(_)) {
                            base.insert(*gate);
                        }
                    }
                    (keep(&base, true), None)
                }
            };
            if let Some(run) = run {
                runs.insert(path.clone(), run);
            }
            guards.insert(path.clone(), guard);
        }
        Guards { guards, runs }
    }

    pub(crate) fn guard(&self, path: &Path) -> BTreeSet<GateId> {
        self.guards.get(path).cloned().unwrap_or_default()
    }

    pub(crate) fn runs(&self, path: &Path) -> BTreeSet<GateId> {
        self.runs.get(path).cloned().unwrap_or_default()
    }

    fn ordered(&self, set: impl IntoIterator<Item = GateId>) -> Vec<GateId> {
        let mut gates: Vec<GateId> = set.into_iter().collect();
        gates.sort_by_key(|gate| (self.depth[*gate], *gate));
        gates
    }

    pub(crate) fn layout(&self, list: Vec<Path>) -> Vec<Item> {
        match self.arrange(list.clone(), &BTreeSet::new()) {
            Some(items) => items,
            None => list.into_iter().map(Item::Node).collect(),
        }
    }

    fn arrange(&self, list: Vec<Path>, open: &BTreeSet<GateId>) -> Option<Vec<Item>> {
        let index: BTreeMap<&Path, usize> = list
            .iter()
            .enumerate()
            .map(|(at, path)| (path, at))
            .collect();
        let mut groups: BTreeMap<GateId, Vec<Path>> = BTreeMap::new();
        let mut parts: Vec<(Option<GateId>, Vec<Path>)> = Vec::new();
        for path in &list {
            let extra = self.ordered(self.guard(path).difference(open).copied());
            match extra.first() {
                None => parts.push((None, vec![path.clone()])),
                Some(gate) => groups.entry(*gate).or_default().push(path.clone()),
            }
        }
        parts.extend(groups.into_iter().map(|(gate, paths)| (Some(gate), paths)));
        let mut owner: BTreeMap<&Path, usize> = BTreeMap::new();
        for (part, (_, paths)) in parts.iter().enumerate() {
            for path in paths {
                owner.insert(path, part);
            }
        }
        let mut waiting = vec![0usize; parts.len()];
        let mut after: Vec<BTreeSet<usize>> = vec![BTreeSet::new(); parts.len()];
        for (part, (_, paths)) in parts.iter().enumerate() {
            let mut needed: BTreeSet<usize> = BTreeSet::new();
            let mut opened: BTreeSet<GateId> = BTreeSet::new();
            for path in paths {
                for (_, from, _) in self.flat.nodes[path].sources() {
                    if let Some(found) = owner.get(from) {
                        needed.insert(*found);
                    }
                }
                opened.extend(self.guard(path).difference(open));
            }
            for gate in opened {
                let branch = &self.branches[gate];
                if let Source::Link(from, _) = self.flat.nodes[&branch.node].input(&branch.control)
                    && let Some(found) = owner.get(from)
                {
                    needed.insert(*found);
                }
            }
            needed.remove(&part);
            waiting[part] = needed.len();
            for found in needed {
                after[found].insert(part);
            }
        }
        let first = |part: usize| -> usize {
            parts[part]
                .1
                .iter()
                .map(|path| index[path])
                .min()
                .unwrap_or(0)
        };
        let mut ready: BTreeSet<(usize, usize)> = (0..parts.len())
            .filter(|part| waiting[*part] == 0)
            .map(|part| (first(part), part))
            .collect();
        let mut sorted = Vec::with_capacity(parts.len());
        while let Some((_, part)) = ready.pop_first() {
            sorted.push(part);
            for next in &after[part] {
                waiting[*next] -= 1;
                if waiting[*next] == 0 {
                    ready.insert((first(*next), *next));
                }
            }
        }
        if sorted.len() != parts.len() {
            return None;
        }
        let mut slots: Vec<Option<(Option<GateId>, Vec<Path>)>> =
            parts.into_iter().map(Some).collect();
        sorted
            .into_iter()
            .map(
                |part| match slots[part].take().expect("each part is placed once") {
                    (None, mut paths) => Some(Item::Node(paths.remove(0))),
                    (Some(gate), paths) => {
                        let mut inner = open.clone();
                        inner.insert(gate);
                        Some(Item::Block(gate, self.arrange(paths, &inner)?))
                    }
                },
            )
            .collect()
    }

    pub(crate) fn condition(
        &self,
        emitter: &mut Emitter<'_, '_>,
        imports: &mut dyn Imports,
        gates: &BTreeSet<GateId>,
    ) -> Condition {
        let mut parts = Vec::new();
        for gate in self.ordered(gates.iter().copied()) {
            match self.test(emitter, imports, gate) {
                Condition::Never => return Condition::Never,
                Condition::Always => {}
                Condition::When(part) => parts.push(part),
            }
        }
        match parts.as_slice() {
            [] => Condition::Always,
            [only] => Condition::When(only.clone()),
            many => Condition::When(
                many.iter()
                    .map(|part| format!("({part})"))
                    .collect::<Vec<_>>()
                    .join(" && "),
            ),
        }
    }

    fn test(
        &self,
        emitter: &mut Emitter<'_, '_>,
        imports: &mut dyn Imports,
        gate: GateId,
    ) -> Condition {
        let branch = &self.branches[gate];
        if let Some(bit) = branch.feature {
            return Condition::When(format!("rikka_feature({bit}u)"));
        }
        let node = &self.flat.nodes[&branch.node];
        imports.import_port(emitter, node, &branch.control);
        let source = node.input(&branch.control).clone();
        match emitter.source(node, &branch.control, &source) {
            Value::Constant(constant) => match branch.test.skips(&constant) {
                true => Condition::Never,
                false => Condition::Always,
            },
            Value::Code(code, ty) => {
                Condition::When(branch.test.runs(&code, ty.width().unwrap_or(1)))
            }
        }
    }

    pub(crate) fn emit(
        &self,
        emitter: &mut Emitter<'_, 'g>,
        imports: &mut dyn Imports,
        items: &[Item],
        depth: usize,
    ) {
        for item in items {
            match item {
                Item::Node(path) => {
                    let node = &self.flat.nodes[path];
                    imports.import(emitter, node);
                    emitter.node(path, node);
                }
                Item::Block(gate, inner) => self.block(emitter, imports, *gate, inner, depth),
            }
        }
    }

    fn block(
        &self,
        emitter: &mut Emitter<'_, 'g>,
        imports: &mut dyn Imports,
        gate: GateId,
        inner: &[Item],
        depth: usize,
    ) {
        let mut members = BTreeSet::new();
        collect(inner, &mut members);
        let escapes: Vec<Slot> = members
            .iter()
            .flat_map(|path| {
                self.users
                    .range((path.clone(), String::new())..)
                    .take_while(move |((from, _), _)| from == path)
            })
            .filter(|(_, users)| users.iter().any(|user| !members.contains(user)))
            .map(|(slot, _)| slot.clone())
            .collect();
        let condition = self.test(emitter, imports, gate);
        if condition == Condition::Always {
            self.emit(emitter, imports, inner, depth);
            return;
        }
        let saved = std::mem::take(&mut emitter.lines);
        if let Condition::When(_) = condition {
            self.emit(emitter, imports, inner, depth + 1);
        }
        let mut body = std::mem::replace(&mut emitter.lines, saved);
        let mut declarations = Vec::new();
        for slot in escapes {
            let ty = match emitter.found(&slot) {
                Some(Value::Constant(_)) => continue,
                Some(Value::Code(_, ty)) => *ty,
                None => match self.flat.output_type(&slot.0, &slot.1) {
                    Some(ty) if condition == Condition::Never => ty,
                    _ => continue,
                },
            };
            let initial = self.initial(emitter, gate, &slot, ty);
            if condition == Condition::Never {
                emitter.seed(slot, initial);
                continue;
            }
            let name = slot_name(&slot, depth);
            declarations.push(format!("{} {name} = {};", ty.slang(), text(&initial)));
            if let Some(value) = emitter.found(&slot) {
                body.push(format!("{name} = {};", text(value)));
            }
            emitter.seed(slot, Value::Code(name, ty));
        }
        if let Condition::When(runs) = condition {
            emitter.lines.extend(declarations);
            emitter.lines.push(format!("if ({runs}) {{"));
            emitter
                .lines
                .extend(body.into_iter().map(|line| format!("    {line}")));
            emitter.lines.push("}".to_owned());
        }
    }

    fn initial(
        &self,
        emitter: &mut Emitter<'_, 'g>,
        gate: GateId,
        slot: &Slot,
        ty: ValueType,
    ) -> Value {
        let branch = &self.branches[gate];
        if branch.node == slot.0
            && let Effect::Node(bypass) = &branch.effect
        {
            let node = &self.flat.nodes[&branch.node];
            return match bypass {
                Bypass::Input(input) => coerce(emitter.argument(node, input), ty),
                Bypass::Constant(constant) => coerce(Value::Constant(*constant), ty),
            };
        }
        Value::Code(zero(ty), ty)
    }
}

fn collect(items: &[Item], members: &mut BTreeSet<Path>) {
    for item in items {
        match item {
            Item::Node(path) => {
                members.insert(path.clone());
            }
            Item::Block(_, inner) => collect(inner, members),
        }
    }
}
