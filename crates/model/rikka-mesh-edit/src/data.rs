use glam::{Vec2, Vec3, Vec4};
use rikka_polymesh::{
    Attribute, AttributeValue, Attributes, Domain, Influence, Influences, Values,
};

pub(crate) const KEY_INDEX_NONE: i32 = -1;

macro_rules! each_values {
    ($values:expr, $list:ident => $body:expr) => {
        match $values {
            Values::Bool($list) => $body,
            Values::Int($list) => $body,
            Values::Float($list) => $body,
            Values::Vec2($list) => $body,
            Values::Vec3($list) => $body,
            Values::Color($list) => $body,
        }
    };
}

pub(crate) use each_values;

#[derive(Clone, Debug)]
pub(crate) struct DataTable {
    pub(crate) domain: Domain,
    pub(crate) attributes: Attributes,
    pub(crate) weights: Option<Vec<Influences>>,
    pub(crate) keys: Vec<Vec<Vec3>>,
    pub(crate) key_index: Vec<i32>,
}

fn mixed<T: AttributeValue>(list: &[T], sources: &[(u32, f32)]) -> T {
    T::mix(
        sources
            .iter()
            .map(|&(index, weight)| (list[index as usize], weight)),
    )
}

fn mix_vec3(list: &[Vec3], sources: &[(u32, f32)]) -> Vec3 {
    sources
        .iter()
        .map(|&(index, weight)| list[index as usize] * weight)
        .sum()
}

pub(crate) fn mix_influences(rows: &[Influences], sources: &[(u32, f32)]) -> Influences {
    let mut mixed: Influences = Vec::new();
    for &(row, share) in sources {
        for influence in &rows[row as usize] {
            let weight = influence.weight * share;
            if weight == 0.0 {
                continue;
            }
            match mixed
                .iter_mut()
                .find(|existing| existing.group == influence.group)
            {
                Some(existing) => existing.weight += weight,
                None => mixed.push(Influence {
                    group: influence.group,
                    weight,
                }),
            }
        }
    }
    mixed
}

fn pick<T: Copy>(list: &[T], indices: &[u32]) -> Vec<T> {
    indices.iter().map(|&index| list[index as usize]).collect()
}

fn set_default(values: &mut Values, index: usize) {
    match values {
        Values::Bool(list) => list[index] = bool::DEFAULT,
        Values::Int(list) => list[index] = i32::DEFAULT,
        Values::Float(list) => list[index] = f32::DEFAULT,
        Values::Vec2(list) => list[index] = Vec2::DEFAULT,
        Values::Vec3(list) => list[index] = Vec3::DEFAULT,
        Values::Color(list) => list[index] = Vec4::DEFAULT,
    }
}

fn mix_across(target: &mut Values, to: usize, source: &Values, sources: &[(u32, f32)]) {
    match (target, source) {
        (Values::Bool(target), Values::Bool(source)) => target[to] = mixed(source, sources),
        (Values::Int(target), Values::Int(source)) => target[to] = mixed(source, sources),
        (Values::Float(target), Values::Float(source)) => target[to] = mixed(source, sources),
        (Values::Vec2(target), Values::Vec2(source)) => target[to] = mixed(source, sources),
        (Values::Vec3(target), Values::Vec3(source)) => target[to] = mixed(source, sources),
        (Values::Color(target), Values::Color(source)) => target[to] = mixed(source, sources),
        _ => {}
    }
}

fn copy_across(target: &mut Values, to: usize, source: &Values, from: usize) {
    match (target, source) {
        (Values::Bool(target), Values::Bool(source)) => target[to] = source[from],
        (Values::Int(target), Values::Int(source)) => target[to] = source[from],
        (Values::Float(target), Values::Float(source)) => target[to] = source[from],
        (Values::Vec2(target), Values::Vec2(source)) => target[to] = source[from],
        (Values::Vec3(target), Values::Vec3(source)) => target[to] = source[from],
        (Values::Color(target), Values::Color(source)) => target[to] = source[from],
        _ => {}
    }
}

impl DataTable {
    pub(crate) fn new(domain: Domain) -> DataTable {
        DataTable {
            domain,
            attributes: Attributes::new(),
            weights: None,
            keys: Vec::new(),
            key_index: Vec::new(),
        }
    }

    fn is_point(&self) -> bool {
        self.domain == Domain::Point
    }

    pub(crate) fn push_default(&mut self, len_before: usize) {
        self.attributes.resize(self.domain, len_before + 1);
        if let Some(weights) = &mut self.weights {
            weights.push(Vec::new());
        }
        for key in &mut self.keys {
            key.push(Vec3::ZERO);
        }
        if self.is_point() {
            self.key_index.push(0);
        }
    }

    pub(crate) fn reset(&mut self, index: usize) {
        for attribute in self.attributes.iter_mut() {
            set_default(&mut attribute.values, index);
        }
        if let Some(weights) = &mut self.weights {
            weights[index].clear();
        }
        for key in &mut self.keys {
            key[index] = Vec3::ZERO;
        }
        if self.is_point() {
            self.key_index[index] = 0;
        }
    }

    pub(crate) fn copy(&mut self, from: usize, to: usize) {
        if from == to {
            return;
        }
        self.attributes.copy(self.domain, from, to);
        if let Some(weights) = &mut self.weights {
            weights[to] = weights[from].clone();
        }
        for key in &mut self.keys {
            key[to] = key[from];
        }
        if self.is_point() {
            self.key_index[to] = self.key_index[from];
        }
    }

    pub(crate) fn mix(&mut self, sources: &[(u32, f32)], to: usize) {
        if sources.is_empty() {
            return;
        }
        self.attributes.interpolate(self.domain, sources, to);
        if let Some(weights) = &mut self.weights {
            weights[to] = mix_influences(weights, sources);
        }
        for key in &mut self.keys {
            key[to] = mix_vec3(key, sources);
        }
    }

    pub(crate) fn copy_from(&mut self, source: &DataTable, from: usize, to: usize) {
        for target in self.attributes.iter_mut() {
            if let Some(source) = source.attributes.get(&target.name) {
                copy_across(&mut target.values, to, &source.values, from);
            }
        }
        if let (Some(weights), Some(source)) = (&mut self.weights, &source.weights) {
            weights[to] = source[from].clone();
        }
        for (key, source) in self.keys.iter_mut().zip(&source.keys) {
            key[to] = source[from];
        }
        if self.is_point() && source.is_point() {
            self.key_index[to] = source.key_index[from];
        }
    }

    pub(crate) fn mix_from(&mut self, source: &DataTable, sources: &[(u32, f32)], to: usize) {
        if sources.is_empty() {
            return;
        }
        for target in self.attributes.iter_mut() {
            if let Some(source) = source.attributes.get(&target.name) {
                mix_across(&mut target.values, to, &source.values, sources);
            }
        }
        if let (Some(weights), Some(source)) = (&mut self.weights, &source.weights) {
            weights[to] = mix_influences(source, sources);
        }
        for (key, source) in self.keys.iter_mut().zip(&source.keys) {
            key[to] = mix_vec3(source, sources);
        }
    }

    pub(crate) fn extract(&self, indices: &[u32]) -> DataTable {
        let mut attributes = Attributes::new();
        for attribute in self.attributes.iter() {
            let values = each_values!(&attribute.values, list => pick_values(list, indices));
            attributes.insert(Attribute::new(
                attribute.name.clone(),
                attribute.domain,
                values,
            ));
        }
        DataTable {
            domain: self.domain,
            attributes,
            weights: self.weights.as_ref().map(|weights| {
                indices
                    .iter()
                    .map(|&index| weights[index as usize].clone())
                    .collect()
            }),
            keys: self.keys.iter().map(|key| pick(key, indices)).collect(),
            key_index: match self.is_point() {
                true => pick(&self.key_index, indices),
                false => Vec::new(),
            },
        }
    }

    pub(crate) fn gather(&mut self, indices: &[u32]) {
        self.attributes.gather(self.domain, indices);
        if let Some(weights) = &mut self.weights {
            *weights = indices
                .iter()
                .map(|&index| std::mem::take(&mut weights[index as usize]))
                .collect();
        }
        for key in &mut self.keys {
            *key = pick(key, indices);
        }
        if self.is_point() {
            self.key_index = pick(&self.key_index, indices);
        }
    }
}

fn pick_values<T: AttributeValue>(list: &[T], indices: &[u32]) -> Values {
    T::wrap(pick(list, indices))
}
