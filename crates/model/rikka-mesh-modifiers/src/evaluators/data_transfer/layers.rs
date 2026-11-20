use glam::{Vec2, Vec4};
use rikka_polymesh::{DataType, Domain, Influence, Layers, Polymesh};

use super::remap::Sources;
use crate::context::Bone;
use crate::kinds::data_transfer::{LayerSelection, LayersDestination, LayersSource, MixMode};

pub(super) struct Mix<'a> {
    pub(super) mode: MixMode,
    pub(super) factor: f32,
    pub(super) weights: Option<&'a [f32]>,
}

impl Mix<'_> {
    pub(super) fn factor_at(&self, element: usize) -> f32 {
        self.factor * self.weights.map_or(1.0, |weights| weights[element])
    }
}

fn mix_float(mode: MixMode, destination: f32, source: f32, factor: f32) -> f32 {
    if (mode == MixMode::ReplaceAboveThreshold && destination < factor)
        || (mode == MixMode::ReplaceBelowThreshold && destination > factor)
    {
        return destination;
    }
    let value = match mode {
        MixMode::ReplaceAboveThreshold | MixMode::ReplaceBelowThreshold => return source,
        MixMode::Mix => (destination + source) * 0.5,
        MixMode::Add => destination + source,
        MixMode::Subtract => destination - source,
        MixMode::Multiply => destination * source,
        MixMode::Replace => source,
    };
    factor * value + (1.0 - factor) * destination
}

fn mix_color(mode: MixMode, destination: Vec4, source: Vec4, factor: f32) -> Vec4 {
    match mode {
        MixMode::ReplaceAboveThreshold | MixMode::ReplaceBelowThreshold => {
            let level = (destination.x + destination.y + destination.z) / 3.0;
            if (mode == MixMode::ReplaceAboveThreshold && level < factor)
                || (mode == MixMode::ReplaceBelowThreshold && level > factor)
            {
                return destination;
            }
            source
        }
        _ => {
            let blended = match mode {
                MixMode::Mix => blend_mix(destination, source),
                MixMode::Add => blend_add(destination, source),
                MixMode::Subtract => blend_subtract(destination, source),
                MixMode::Multiply => blend_multiply(destination, source),
                _ => source,
            };
            destination * (1.0 - factor) + blended * factor
        }
    }
}

fn blend_mix(first: Vec4, second: Vec4) -> Vec4 {
    if second.w == 0.0 {
        return first;
    }
    let t = second.w;
    let mt = 1.0 - t;
    Vec4::new(
        mt * first.x + second.x,
        mt * first.y + second.y,
        mt * first.z + second.z,
        mt * first.w + t,
    )
}

fn blend_add(first: Vec4, second: Vec4) -> Vec4 {
    if second.w == 0.0 {
        return first;
    }
    Vec4::new(
        first.x + second.x * first.w,
        first.y + second.y * first.w,
        first.z + second.z * first.w,
        first.w,
    )
}

fn blend_subtract(first: Vec4, second: Vec4) -> Vec4 {
    if second.w == 0.0 {
        return first;
    }
    Vec4::new(
        (first.x - second.x * first.w).max(0.0),
        (first.y - second.y * first.w).max(0.0),
        (first.z - second.z * first.w).max(0.0),
        first.w,
    )
}

fn blend_multiply(first: Vec4, second: Vec4) -> Vec4 {
    if second.w == 0.0 {
        return first;
    }
    let t = second.w;
    let mt = 1.0 - t;
    Vec4::new(
        mt * first.x + first.x * second.x * first.w,
        mt * first.y + first.y * second.y * first.w,
        mt * first.z + first.z * second.z * first.w,
        first.w,
    )
}

pub(super) fn transfer_floats(
    source: &Polymesh,
    mesh: &mut Polymesh,
    map: &[Sources],
    name: &str,
    domain: Domain,
) {
    let Some(values) = source.attributes.values::<f32>(name, domain) else {
        return;
    };
    let Some(target) = mesh.attributes.values_mut::<f32>(name, domain) else {
        return;
    };
    for (element, sources) in map.iter().enumerate() {
        if sources.is_empty() {
            continue;
        }
        target[element] = sources
            .iter()
            .map(|&(index, weight)| values[index as usize] * weight)
            .sum();
    }
}

pub(super) fn transfer_bools(
    source: &Polymesh,
    mesh: &mut Polymesh,
    map: &[Sources],
    name: &str,
    domain: Domain,
) {
    let Some(values) = source.attributes.values::<bool>(name, domain) else {
        return;
    };
    if mesh.attributes.get(name).is_none() {
        let len = mesh.domain_len(domain);
        mesh.attributes.ensure::<bool>(name, domain, len);
    }
    let Some(target) = mesh.attributes.values_mut::<bool>(name, domain) else {
        return;
    };
    for (element, sources) in map.iter().enumerate() {
        if sources.is_empty() {
            continue;
        }
        target[element] = sources
            .iter()
            .any(|&(index, weight)| values[index as usize] && weight > 0.0);
    }
}

fn layer_names(mesh: &Polymesh, data_type: DataType, domain: Domain) -> Vec<String> {
    mesh.attributes
        .iter()
        .filter(|attribute| attribute.domain == domain && attribute.data_type() == data_type)
        .map(|attribute| attribute.name.clone())
        .collect()
}

fn active_layer(mesh: &Polymesh, data_type: DataType) -> Option<String> {
    let layers = match data_type {
        DataType::Vec2 => &mesh.uv_maps,
        _ => &mesh.color_attributes,
    };
    layers.active_name().map(str::to_owned)
}

fn has_layer(mesh: &Polymesh, name: &str, data_type: DataType, domain: Domain) -> bool {
    mesh.attributes
        .get(name)
        .is_some_and(|attribute| attribute.domain == domain && attribute.data_type() == data_type)
}

pub(super) fn transfer_layers(
    source: &Polymesh,
    mesh: &mut Polymesh,
    map: &[Sources],
    data_type: DataType,
    domain: Domain,
    selection: LayerSelection,
    mix: &Mix<'_>,
) {
    let source_names = layer_names(source, data_type, domain);
    if source_names.is_empty() {
        return;
    }
    let destination_names = layer_names(mesh, data_type, domain);
    let mut pairs: Vec<(String, String)> = Vec::new();
    match selection.source {
        LayersSource::Active | LayersSource::Layer(_) => {
            let name = match selection.source {
                LayersSource::Layer(index) => match source_names.get(index as usize) {
                    Some(name) => name.clone(),
                    None => return,
                },
                _ => match active_layer(source, data_type) {
                    Some(name) if has_layer(source, &name, data_type, domain) => name,
                    _ => return,
                },
            };
            let target = match selection.destination {
                LayersDestination::Layer(index) => match destination_names.get(index as usize) {
                    Some(name) => name.clone(),
                    None => return,
                },
                LayersDestination::Active => match active_layer(mesh, data_type) {
                    Some(target) if has_layer(mesh, &target, data_type, domain) => target,
                    _ => return,
                },
                LayersDestination::Index => return,
                LayersDestination::Name => match has_layer(mesh, &name, data_type, domain) {
                    true => name.clone(),
                    false => return,
                },
            };
            transfer_names(mesh, source, data_type);
            pairs.push((name, target));
        }
        LayersSource::All => {
            match selection.destination {
                LayersDestination::Index => {
                    let count = source_names.len().min(destination_names.len());
                    for index in (0..count).rev() {
                        pairs.push((
                            source_names[index].clone(),
                            destination_names[index].clone(),
                        ));
                    }
                }
                LayersDestination::Name => {
                    for name in source_names.iter().rev() {
                        if has_layer(mesh, name, data_type, domain) {
                            pairs.push((name.clone(), name.clone()));
                        }
                    }
                }
                LayersDestination::Active | LayersDestination::Layer(_) => return,
            }
            transfer_names(mesh, source, data_type);
        }
        LayersSource::BoneSelected | LayersSource::BoneDeform => return,
    }
    for (from, to) in pairs {
        match data_type {
            DataType::Vec2 => mix_layer::<Vec2>(source, mesh, map, &from, &to, domain, mix),
            _ => mix_layer::<Vec4>(source, mesh, map, &from, &to, domain, mix),
        }
    }
}

trait Mixable:
    rikka_polymesh::AttributeValue + std::ops::Mul<f32, Output = Self> + std::ops::Add<Output = Self>
{
    const ZERO: Self;
    fn blend(mode: MixMode, destination: Self, source: Self, factor: f32) -> Self;
}

impl Mixable for Vec2 {
    const ZERO: Vec2 = Vec2::ZERO;

    fn blend(_: MixMode, destination: Vec2, source: Vec2, factor: f32) -> Vec2 {
        destination * (1.0 - factor) + source * factor
    }
}

impl Mixable for Vec4 {
    const ZERO: Vec4 = Vec4::ZERO;

    fn blend(mode: MixMode, destination: Vec4, source: Vec4, factor: f32) -> Vec4 {
        mix_color(mode, destination, source, factor)
    }
}

fn mix_layer<T: Mixable>(
    source: &Polymesh,
    mesh: &mut Polymesh,
    map: &[Sources],
    from: &str,
    to: &str,
    domain: Domain,
    mix: &Mix<'_>,
) {
    let Some(values) = source
        .attributes
        .values::<T>(from, domain)
        .map(<[T]>::to_vec)
    else {
        return;
    };
    let Some(target) = mesh.attributes.values_mut::<T>(to, domain) else {
        return;
    };
    for (element, sources) in map.iter().enumerate() {
        if sources.is_empty() {
            continue;
        }
        let value = sources.iter().fold(T::ZERO, |sum, &(index, weight)| {
            sum + values[index as usize] * weight
        });
        target[element] = T::blend(mix.mode, target[element], value, mix.factor_at(element));
    }
}

fn transfer_names(mesh: &mut Polymesh, source: &Polymesh, data_type: DataType) {
    let domains: &[Domain] = match data_type {
        DataType::Vec2 => &[Domain::Corner],
        _ => &[Domain::Point, Domain::Corner],
    };
    let candidates: Vec<String> = mesh
        .attributes
        .iter()
        .filter(|attribute| {
            attribute.data_type() == data_type && domains.contains(&attribute.domain)
        })
        .map(|attribute| attribute.name.clone())
        .collect();
    let is_layer = |mesh: &Polymesh, name: &str| {
        mesh.attributes.get(name).is_some_and(|attribute| {
            attribute.data_type() == data_type && domains.contains(&attribute.domain)
        })
    };
    let (source_layers, layers) = match data_type {
        DataType::Vec2 => (&source.uv_maps, &mut mesh.uv_maps),
        _ => (&source.color_attributes, &mut mesh.color_attributes),
    };
    let pick = |layers: &mut Layers, wanted: Option<&str>, active: bool| {
        let current = match active {
            true => layers.active_name(),
            false => layers.render_name(),
        };
        if current.is_some() {
            return;
        }
        let Some(wanted) = wanted else {
            return;
        };
        let chosen = match candidates.iter().any(|name| name == wanted) {
            true => Some(wanted.to_owned()),
            false => candidates.first().cloned(),
        };
        if let Some(chosen) = chosen {
            if !layers.contains(&chosen) {
                layers.push(chosen.clone());
            }
            match active {
                true => layers.set_active(&chosen),
                false => layers.set_render(&chosen),
            };
        }
    };
    let source_active = source_layers
        .active_name()
        .filter(|name| is_layer(source, name))
        .map(str::to_owned);
    let source_render = source_layers
        .render_name()
        .filter(|name| is_layer(source, name))
        .map(str::to_owned);
    pick(layers, source_active.as_deref(), true);
    pick(layers, source_render.as_deref(), false);
}

pub(super) fn transfer_vertex_groups(
    source: &Polymesh,
    mesh: &mut Polymesh,
    map: &[Sources],
    selection: LayerSelection,
    mix: &Mix<'_>,
    bones: Option<Vec<Bone>>,
) {
    let source_groups = &source.vertex_groups.groups;
    if source_groups.is_empty() {
        return;
    }
    let destination_count = mesh.vertex_groups.groups.len();
    let mut pairs: Vec<(u32, u32)> = Vec::new();
    match selection.source {
        LayersSource::Active | LayersSource::Layer(_) => {
            let from = match selection.source {
                LayersSource::Layer(index) if (index as usize) < source_groups.len() => index,
                LayersSource::Active => match source.vertex_groups.active {
                    Some(active) => active as u32,
                    None => return,
                },
                _ => return,
            };
            let to = match selection.destination {
                LayersDestination::Layer(index) => index,
                LayersDestination::Active => match mesh.vertex_groups.active {
                    Some(active) => active as u32,
                    None => return,
                },
                LayersDestination::Index => from,
                LayersDestination::Name => {
                    match mesh.vertex_groups.index(&source_groups[from as usize].name) {
                        Some(index) => index,
                        None => return,
                    }
                }
            };
            if (to as usize) >= destination_count {
                return;
            }
            pairs.push((from, to));
        }
        LayersSource::All | LayersSource::BoneSelected | LayersSource::BoneDeform => {
            let used: Vec<bool> = match selection.source {
                LayersSource::All => vec![true; source_groups.len()],
                _ => {
                    let Some(bones) = bones else {
                        return;
                    };
                    source_groups
                        .iter()
                        .map(|group| {
                            bones.iter().any(|bone| {
                                bone.name == group.name
                                    && match selection.source {
                                        LayersSource::BoneSelected => bone.selected,
                                        _ => bone.deform,
                                    }
                            })
                        })
                        .collect()
                }
            };
            match selection.destination {
                LayersDestination::Index => {
                    let last = used
                        .iter()
                        .rposition(|&flag| flag)
                        .map_or(0, |last| last + 1);
                    let count = last.min(destination_count);
                    for index in (0..count).rev() {
                        if used[index] {
                            pairs.push((index as u32, index as u32));
                        }
                    }
                }
                LayersDestination::Name => {
                    for (index, group) in source_groups.iter().enumerate() {
                        if !used[index] {
                            continue;
                        }
                        if let Some(to) = mesh.vertex_groups.index(&group.name) {
                            pairs.push((index as u32, to));
                        }
                    }
                }
                LayersDestination::Active | LayersDestination::Layer(_) => return,
            }
        }
    }
    if pairs.is_empty() {
        return;
    }
    mesh.vertex_groups.ensure_weights(mesh.point_count());
    for (from, to) in pairs {
        for (point, sources) in map.iter().enumerate() {
            if sources.is_empty() {
                continue;
            }
            let mut weight_source = 0.0f32;
            let mut found = false;
            for &(index, weight) in sources.iter().rev() {
                let entry =
                    source
                        .vertex_groups
                        .weights
                        .get(index as usize)
                        .and_then(|influences| {
                            influences.iter().find(|influence| influence.group == from)
                        });
                if let Some(entry) = entry {
                    weight_source += entry.weight * weight;
                    found = true;
                }
            }
            let influences = &mut mesh.vertex_groups.weights[point];
            let existing = influences
                .iter()
                .position(|influence| influence.group == to);
            let weight_destination = match existing {
                Some(slot) => influences[slot].weight,
                None if mix.mode == MixMode::ReplaceAboveThreshold => continue,
                None => 0.0,
            };
            let mixed = mix_float(
                mix.mode,
                weight_destination,
                weight_source,
                mix.factor_at(point),
            )
            .clamp(0.0, 1.0);
            match (found, existing) {
                (false, Some(slot)) | (true, Some(slot)) => influences[slot].weight = mixed,
                (false, None) => {}
                (true, None) => influences.push(Influence {
                    group: to,
                    weight: mixed,
                }),
            }
        }
    }
}
