use std::collections::HashMap;

use rikka_mmd::{Pmx, PmxPanel};
use rikka_model_core::{Instance, Model, NodeId, Pose};
use rikka_vrm::{Avatar, ExpressionPreset, Vrm};

use crate::expression::{EXPRESSION_COUNT, Expression, MorphWeights};
use crate::names::{Names, normalized};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Panel {
    Eyebrow,
    Eye,
    Mouth,
    Other,
}

impl Panel {
    pub const ALL: [Self; 4] = [Self::Eyebrow, Self::Eye, Self::Mouth, Self::Other];

    fn from_pmx(panel: PmxPanel) -> Option<Self> {
        match panel {
            PmxPanel::Eyebrow => Some(Self::Eyebrow),
            PmxPanel::Eye => Some(Self::Eye),
            PmxPanel::Mouth => Some(Self::Mouth),
            PmxPanel::Other => Some(Self::Other),
            PmxPanel::Hidden => None,
        }
    }

    fn from_vrm(preset: Option<ExpressionPreset>) -> Self {
        match preset {
            Some(
                ExpressionPreset::Blink
                | ExpressionPreset::BlinkLeft
                | ExpressionPreset::BlinkRight
                | ExpressionPreset::LookUp
                | ExpressionPreset::LookDown
                | ExpressionPreset::LookLeft
                | ExpressionPreset::LookRight,
            ) => Self::Eye,
            Some(
                ExpressionPreset::Aa
                | ExpressionPreset::Ih
                | ExpressionPreset::Ou
                | ExpressionPreset::Ee
                | ExpressionPreset::Oh,
            ) => Self::Mouth,
            _ => Self::Other,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Term {
    target: usize,
    morph: usize,
    weight: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Morph {
    name: String,
    loose: String,
    panel: Option<Panel>,
    terms: Vec<Term>,
}

impl Morph {
    fn new(name: String, panel: Option<Panel>, terms: Vec<Term>) -> Self {
        Self {
            name,
            loose: String::new(),
            panel,
            terms,
        }
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn panel(&self) -> Option<Panel> {
        self.panel
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Share {
    morph: usize,
    weight: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Driver {
    #[default]
    Pose,
    Avatar,
}

#[derive(Clone, Debug, Default)]
pub struct Faces {
    targets: Vec<(NodeId, usize)>,
    morphs: Vec<Morph>,
    presets: [Vec<Share>; EXPRESSION_COUNT],
    exact: HashMap<String, usize>,
    loose: HashMap<String, usize>,
    driver: Driver,
}

type Recipe = &'static [(&'static [&'static str], f32)];

fn recipe(expression: Expression) -> Recipe {
    match expression {
        Expression::Happy => &[(&["笑い"], 1.0), (&["にこり"], 1.0)],
        Expression::Angry => &[(&["怒り"], 1.0), (&["ｷﾘｯ"], 0.6)],
        Expression::Sad => &[(&["困る"], 1.0), (&["じと目"], 0.3)],
        Expression::Relaxed => &[(&["なごみ"], 1.0), (&["にこり"], 0.5)],
        Expression::Surprised => &[(&["びっくり"], 1.0), (&["上"], 0.8)],
        Expression::Blink => &[(&["まばたき"], 1.0)],
        Expression::BlinkLeft => &[(&["ウィンク", "ウィンク２"], 1.0)],
        Expression::BlinkRight => &[(&["ウィンク右", "ｳｨﾝｸ２右"], 1.0)],
        Expression::Aa => &[(&["あ"], 1.0)],
        Expression::Ih => &[(&["い"], 1.0)],
        Expression::Ou => &[(&["う"], 1.0)],
        Expression::Ee => &[(&["え"], 1.0)],
        Expression::Oh => &[(&["お"], 1.0)],
    }
}

impl Faces {
    #[must_use]
    pub fn from_model(model: &Model) -> Self {
        let faces = if let Some(vrm) = model.extensions.get::<Vrm>() {
            Self::from_vrm(model, vrm)
        } else if let Some(pmx) = model.extensions.get::<Pmx>() {
            Self::from_pmx(pmx)
        } else {
            Self::from_meshes(model)
        };
        faces.indexed()
    }

    fn from_pmx(pmx: &Pmx) -> Self {
        let mut faces = Self {
            targets: vec![(pmx.root, pmx.morphs.len())],
            morphs: pmx
                .morphs
                .iter()
                .enumerate()
                .map(|(morph, index)| {
                    Morph::new(
                        index.name.clone(),
                        Panel::from_pmx(index.panel),
                        vec![Term {
                            target: 0,
                            morph,
                            weight: 1.0,
                        }],
                    )
                })
                .collect(),
            ..Self::default()
        };
        let names = Names::new(pmx.morphs.iter().map(|morph| morph.name.as_str()));
        for expression in Expression::ALL {
            faces.presets[expression.index()] = recipe(expression)
                .iter()
                .filter_map(|(choices, weight)| {
                    let choices: Vec<String> =
                        choices.iter().map(|choice| (*choice).to_owned()).collect();
                    Some(Share {
                        morph: names.find(&choices)?,
                        weight: *weight,
                    })
                })
                .collect();
        }
        faces
    }

    fn from_vrm(model: &Model, vrm: &Vrm) -> Self {
        let mut faces = Self {
            targets: morph_targets(model)
                .into_iter()
                .map(|(node, names)| (node, names.len()))
                .collect(),
            driver: Driver::Avatar,
            ..Self::default()
        };
        for vrm_expression in &vrm.expressions {
            let terms = vrm_expression
                .morph_binds
                .iter()
                .filter_map(|bind| {
                    let target = faces.targets.iter().position(|(node, count)| {
                        *node == bind.node && (bind.index as usize) < *count
                    })?;
                    Some(Term {
                        target,
                        morph: bind.index as usize,
                        weight: bind.weight,
                    })
                })
                .collect();
            let preset = vrm_expression.name.preset();
            if let Some(expression) = Expression::ALL
                .iter()
                .find(|expression| preset.is_some_and(|preset| preset.name() == expression.name()))
            {
                faces.presets[expression.index()] = vec![Share {
                    morph: faces.morphs.len(),
                    weight: 1.0,
                }];
            }
            faces.morphs.push(Morph::new(
                vrm_expression.name.as_str().to_owned(),
                Some(Panel::from_vrm(preset)),
                terms,
            ));
        }
        faces
    }

    fn from_meshes(model: &Model) -> Self {
        let targets = morph_targets(model);
        let mut faces = Self {
            targets: targets
                .iter()
                .map(|(node, names)| (*node, names.len()))
                .collect(),
            ..Self::default()
        };
        let mut index_by_name: HashMap<String, usize> = HashMap::new();
        for (target, (_, names)) in targets.into_iter().enumerate() {
            for (morph, name) in names.into_iter().enumerate() {
                let term = Term {
                    target,
                    morph,
                    weight: 1.0,
                };
                match index_by_name.get(&name) {
                    Some(at) => faces.morphs[*at].terms.push(term),
                    None => {
                        index_by_name.insert(name.clone(), faces.morphs.len());
                        faces
                            .morphs
                            .push(Morph::new(name, Some(Panel::Other), vec![term]));
                    }
                }
            }
        }
        for (at, morph) in faces.morphs.iter().enumerate() {
            if let Some(expression) = Expression::recognize(&morph.name)
                && faces.presets[expression.index()].is_empty()
            {
                faces.presets[expression.index()] = vec![Share {
                    morph: at,
                    weight: 1.0,
                }];
            }
        }
        faces
    }

    fn indexed(mut self) -> Self {
        for (at, morph) in self.morphs.iter_mut().enumerate() {
            morph.loose = normalized(&morph.name);
            self.exact.entry(morph.name.clone()).or_insert(at);
            self.loose.entry(morph.loose.clone()).or_insert(at);
        }
        self
    }

    #[must_use]
    pub fn morphs(&self) -> &[Morph] {
        &self.morphs
    }

    #[must_use]
    pub fn morph(&self, name: &str) -> Option<&Morph> {
        self.index(name).map(|at| &self.morphs[at])
    }

    #[must_use]
    pub fn index(&self, name: &str) -> Option<usize> {
        self.exact.get(name).copied()
    }

    #[must_use]
    pub fn resolve(&self, name: &str) -> Option<&str> {
        self.exact
            .get(name)
            .or_else(|| self.loose.get(&normalized(name)))
            .map(|at| self.morphs[*at].name.as_str())
    }

    #[must_use]
    pub fn adopt(&self, name: &str) -> Vec<(&str, f32)> {
        if let Some(resolved) = self.resolve(name) {
            return vec![(resolved, 1.0)];
        }
        Expression::recognize(name).map_or_else(Vec::new, |expression| {
            self.presets[expression.index()]
                .iter()
                .map(|share| (self.morphs[share.morph].name.as_str(), share.weight))
                .collect()
        })
    }

    pub fn search<'a>(&'a self, query: &str) -> impl Iterator<Item = &'a Morph> {
        let query = normalized(query);
        self.morphs
            .iter()
            .filter(move |morph| morph.loose.contains(&query))
    }

    #[must_use]
    pub fn has(&self, expression: Expression) -> bool {
        !self.presets[expression.index()].is_empty()
    }

    #[must_use]
    pub fn strength(&self, weights: &MorphWeights, expression: Expression) -> f32 {
        self.strength_by(expression, |morph| weights.weight(&self.morphs[morph].name))
    }

    pub fn express(&self, weights: &mut MorphWeights, expression: Expression, strength: f32) {
        self.express_by(expression, strength, |morph, weight| {
            weights.set(&self.morphs[morph].name, weight);
        });
    }

    #[must_use]
    pub fn amounts(&self) -> MorphAmounts {
        MorphAmounts(vec![0.0; self.morphs.len()])
    }

    #[must_use]
    pub fn strength_in(&self, amounts: &MorphAmounts, expression: Expression) -> f32 {
        self.strength_by(expression, |morph| amounts.amount(morph))
    }

    pub fn express_in(&self, amounts: &mut MorphAmounts, expression: Expression, strength: f32) {
        self.express_by(expression, strength, |morph, weight| {
            amounts.set(morph, weight)
        });
    }

    fn strength_by(&self, expression: Expression, amount: impl Fn(usize) -> f32) -> f32 {
        self.presets[expression.index()]
            .first()
            .map_or(0.0, |share| amount(share.morph) / share.weight)
    }

    fn express_by(&self, expression: Expression, strength: f32, mut set: impl FnMut(usize, f32)) {
        for share in &self.presets[expression.index()] {
            set(share.morph, share.weight * strength);
        }
    }

    pub fn write(&self, weights: &MorphWeights, instance: &mut Instance) {
        let mut amounts = self.amounts();
        for (name, amount) in weights.iter() {
            if let Some(at) = self.index(name) {
                amounts.set(at, amount);
            }
        }
        self.write_amounts(&amounts, instance);
    }

    pub fn write_amounts(&self, amounts: &MorphAmounts, instance: &mut Instance) {
        match self.driver {
            Driver::Avatar => {
                if let Some(avatar) = instance.rig_as_mut::<Avatar>() {
                    for (expression, amount) in amounts.0.iter().enumerate() {
                        avatar.set_weight(expression, *amount);
                    }
                }
            }
            Driver::Pose => self.write_weights(amounts, instance.pose_mut()),
        }
    }

    fn write_weights(&self, amounts: &MorphAmounts, pose: &mut Pose) {
        for (node, count) in &self.targets {
            if let Some(slot) = pose.weights_mut(*node) {
                slot.clear();
                slot.resize(*count, 0.0);
            }
        }
        for (morph, amount) in self.morphs.iter().zip(&amounts.0) {
            if *amount == 0.0 {
                continue;
            }
            for term in &morph.terms {
                let (node, _) = self.targets[term.target];
                if let Some(weight) = pose
                    .weights_mut(node)
                    .and_then(|slot| slot.get_mut(term.morph))
                {
                    *weight += term.weight * amount;
                }
            }
        }
        for (node, _) in &self.targets {
            if let Some(slot) = pose.weights_mut(*node) {
                for weight in slot {
                    *weight = weight.clamp(0.0, 1.0);
                }
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct MorphAmounts(Vec<f32>);

impl MorphAmounts {
    #[must_use]
    pub fn amount(&self, morph: usize) -> f32 {
        self.0.get(morph).copied().unwrap_or(0.0)
    }

    pub fn set(&mut self, morph: usize, amount: f32) {
        if let Some(slot) = self.0.get_mut(morph) {
            *slot = amount;
        }
    }
}

fn morph_targets(model: &Model) -> Vec<(NodeId, Vec<String>)> {
    model
        .nodes
        .iter()
        .enumerate()
        .filter_map(|(index, node)| {
            let targets = &model.mesh(node.mesh?).morph_targets;
            (!targets.is_empty()).then(|| {
                (
                    NodeId(index as u32),
                    targets.iter().map(|target| target.name.clone()).collect(),
                )
            })
        })
        .collect()
}
