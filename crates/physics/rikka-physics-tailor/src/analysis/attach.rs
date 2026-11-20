use std::collections::{BTreeMap, BTreeSet};

use rikka_model_core::Bounds;

use super::features::MeshFeatures;
use super::parts::Part;
use crate::cloth_settings::Role;
use crate::geometry::{PointGrid, Union, groups};

const TOUCH: f32 = 0.01;
const LYING: f32 = 0.015;
const TOUCH_LEAST: usize = 4;
const TOUCH_SHARE: f32 = 0.05;
const LYING_SHARE: f32 = 0.5;
const SHARED_BONES: f32 = 0.5;
const ATTACH_REACH: f32 = 0.08;
const ATTACH_EXTENT: f32 = 0.3;

#[derive(Clone, Copy, Debug)]
struct Contact {
    other: usize,
    touching: usize,
    lying: usize,
}

pub(crate) fn attach(
    features: &MeshFeatures<'_, '_>,
    parts: &[Part],
    roles: &[Role],
) -> Vec<Vec<usize>> {
    let sheets: Vec<usize> = (0..parts.len())
        .filter(|part| !parts[*part].strands)
        .collect();
    let mut owner = vec![usize::MAX; features.figure.vertices.len()];
    for part in &sheets {
        for vertex in &parts[*part].members {
            owner[*vertex as usize] = *part;
        }
    }
    let grid = PointGrid::new(
        sheets
            .iter()
            .flat_map(|part| parts[*part].members.iter())
            .map(|vertex| (*vertex, features.position(*vertex))),
        LYING * 2.0,
    );
    let bones: Vec<BTreeSet<u32>> = parts
        .iter()
        .map(|part| moving_bones(features, &part.members, roles))
        .collect();
    let joined = |guest: usize, contact: &Contact| {
        let size = parts[guest].members.len();
        let attached = contact.touching >= TOUCH_LEAST.max((TOUCH_SHARE * size as f32) as usize)
            || contact.lying as f32 >= LYING_SHARE * size as f32;
        attached
            && agree(
                features,
                &parts[guest].members,
                roles,
                &bones[contact.other],
            )
    };
    let mut union = Union::new(parts.len());
    let mut objects = Union::new(parts.len());
    let mut contacts: Vec<Vec<Contact>> = vec![Vec::new(); parts.len()];
    for guest in &sheets {
        contacts[*guest] = touching(features, &grid, &owner, *guest, &parts[*guest].members);
        for contact in contacts[*guest]
            .iter()
            .filter(|contact| joined(*guest, contact))
        {
            let host = contact.other;
            let larger = (parts[host].members.len(), std::cmp::Reverse(host))
                > (parts[*guest].members.len(), std::cmp::Reverse(*guest));
            match (parts[*guest].is_attachment, parts[host].is_attachment) {
                (false, false) if larger => union.join(*guest as u32, host as u32),
                (true, true) => objects.join(*guest as u32, host as u32),
                _ => {}
            }
        }
    }
    let (labels, count) = objects.labels();
    for object in groups(&labels, count) {
        if !parts[object[0] as usize].is_attachment || parts[object[0] as usize].strands {
            continue;
        }
        let vertices: Vec<u32> = object
            .iter()
            .flat_map(|part| parts[*part as usize].members.iter().copied())
            .collect();
        if Bounds::from_points(vertices.iter().map(|vertex| features.position(*vertex))).longest()
            > ATTACH_EXTENT
        {
            continue;
        }
        let mut hosts: BTreeMap<usize, usize> = BTreeMap::new();
        for part in &object {
            for contact in &contacts[*part as usize] {
                let host = contact.other;
                if !parts[host].is_attachment
                    && agree(
                        features,
                        &parts[*part as usize].members,
                        roles,
                        &bones[host],
                    )
                {
                    *hosts.entry(host).or_default() += contact.touching + contact.lying;
                }
            }
        }
        let Some(host) = hosts
            .into_iter()
            .max_by(|a, b| a.1.cmp(&b.1).then(b.0.cmp(&a.0)))
            .map(|(host, _)| host)
        else {
            continue;
        };
        if reaches(features, &grid, &owner, &vertices, host) {
            for part in &object {
                union.join(*part, host as u32);
            }
        }
    }
    let (labels, count) = union.labels();
    groups(&labels, count)
        .into_iter()
        .map(|garment| garment.into_iter().map(|part| part as usize).collect())
        .collect()
}

fn touching(
    features: &MeshFeatures<'_, '_>,
    grid: &PointGrid,
    owner: &[usize],
    guest: usize,
    members: &[u32],
) -> Vec<Contact> {
    let mut counts: BTreeMap<usize, (usize, usize)> = BTreeMap::new();
    for vertex in members {
        let at = features.position(*vertex);
        let mut closest: BTreeMap<usize, f32> = BTreeMap::new();
        for other in grid.near(at, LYING) {
            let host = owner[other as usize];
            let distance = features.position(other).distance(at);
            if host != guest && distance <= LYING {
                let best = closest.entry(host).or_insert(f32::MAX);
                *best = best.min(distance);
            }
        }
        for (host, distance) in closest {
            let counted = counts.entry(host).or_default();
            counted.1 += 1;
            if distance <= TOUCH {
                counted.0 += 1;
            }
        }
    }
    counts
        .into_iter()
        .map(|(other, (touching, lying))| Contact {
            other,
            touching,
            lying,
        })
        .collect()
}

fn reaches(
    features: &MeshFeatures<'_, '_>,
    grid: &PointGrid,
    owner: &[usize],
    vertices: &[u32],
    host: usize,
) -> bool {
    vertices.iter().all(|vertex| {
        let at = features.position(*vertex);
        grid.near(at, ATTACH_REACH).any(|other| {
            owner[other as usize] == host && features.position(other).distance(at) <= ATTACH_REACH
        })
    })
}

fn moving_bones(features: &MeshFeatures<'_, '_>, members: &[u32], roles: &[Role]) -> BTreeSet<u32> {
    members
        .iter()
        .filter(|vertex| roles[**vertex as usize] == Role::Free)
        .map(|vertex| features.figure.vertices[*vertex as usize].strongest().0)
        .collect()
}

fn agree(
    features: &MeshFeatures<'_, '_>,
    members: &[u32],
    roles: &[Role],
    bones: &BTreeSet<u32>,
) -> bool {
    let free: Vec<u32> = members
        .iter()
        .copied()
        .filter(|vertex| roles[*vertex as usize] == Role::Free)
        .collect();
    let shared = free
        .iter()
        .filter(|vertex| bones.contains(&features.figure.vertices[**vertex as usize].strongest().0))
        .count();
    shared as f32 >= SHARED_BONES * free.len() as f32
}
