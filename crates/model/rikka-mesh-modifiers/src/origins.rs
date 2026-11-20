use rikka_polymesh::{Attribute, Domain, Polymesh, Values};

const ORIGIN_POINT: &str = ".rikka_origin_point";
const ORIGIN_EDGE: &str = ".rikka_origin_edge";
const ORIGIN_FACE: &str = ".rikka_origin_face";
const CONTROL_EDGE: &str = ".rikka_control_edge";
const CHECK_POINT: &str = ".rikka_origin_check_point";
const CHECK_EDGE: &str = ".rikka_origin_check_edge";
const CHECK_FACE: &str = ".rikka_origin_check_face";
const CHECK_FACTOR: u32 = 0x9E37_79B1;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Origins {
    pub points: Vec<Option<u32>>,
    pub edges: Vec<Option<u32>>,
    pub faces: Vec<Option<u32>>,
}

impl Origins {
    #[must_use]
    pub fn identity(mesh: &Polymesh) -> Origins {
        let range = |len: usize| (0..len as u32).map(Some).collect();
        Origins {
            points: range(mesh.point_count()),
            edges: range(mesh.edge_count()),
            faces: range(mesh.face_count()),
        }
    }
}

fn name(domain: Domain) -> &'static str {
    match domain {
        Domain::Point => ORIGIN_POINT,
        Domain::Edge => ORIGIN_EDGE,
        Domain::Face | Domain::Corner => ORIGIN_FACE,
    }
}

pub(crate) fn attach(mesh: &mut Polymesh) {
    for domain in [Domain::Point, Domain::Edge, Domain::Face] {
        let len = mesh.domain_len(domain) as i32;
        mesh.attributes.insert(Attribute::new(
            name(domain),
            domain,
            Values::Int((1..=len).collect()),
        ));
    }
}

pub(crate) fn detach(mesh: &mut Polymesh) -> (Origins, Option<Vec<bool>>) {
    let mut take = |domain: Domain| -> Vec<Option<u32>> {
        let len = mesh.domain_len(domain);
        let values = mesh
            .attributes
            .remove(name(domain))
            .filter(|attribute| attribute.domain == domain && attribute.values.len() == len)
            .and_then(|attribute| match attribute.values {
                Values::Int(values) => Some(values),
                _ => None,
            })
            .unwrap_or_else(|| vec![0; len]);
        values
            .into_iter()
            .map(|value| u32::try_from(value - 1).ok())
            .collect()
    };
    let origins = Origins {
        points: take(Domain::Point),
        edges: take(Domain::Edge),
        faces: take(Domain::Face),
    };
    let control = mesh
        .attributes
        .remove(CONTROL_EDGE)
        .filter(|attribute| {
            attribute.domain == Domain::Edge && attribute.values.len() == mesh.edge_count()
        })
        .and_then(|attribute| match attribute.values {
            Values::Bool(values) => Some(values),
            _ => None,
        });
    (origins, control)
}

pub(crate) fn strip(mesh: &mut Polymesh) {
    for attribute in [ORIGIN_POINT, ORIGIN_EDGE, ORIGIN_FACE, CONTROL_EDGE] {
        mesh.attributes.remove(attribute);
    }
}

fn check_name(domain: Domain) -> &'static str {
    match domain {
        Domain::Point => CHECK_POINT,
        Domain::Edge => CHECK_EDGE,
        Domain::Face | Domain::Corner => CHECK_FACE,
    }
}

fn check_value(value: i32) -> i32 {
    ((value as u32).wrapping_mul(CHECK_FACTOR) >> 8) as i32
}

pub(crate) fn seal(mesh: &mut Polymesh) {
    for domain in [Domain::Point, Domain::Edge, Domain::Face] {
        let Some(values) = mesh.attributes.values::<i32>(name(domain), domain) else {
            continue;
        };
        let checks = values.iter().map(|&value| check_value(value)).collect();
        mesh.attributes.insert(Attribute::new(
            check_name(domain),
            domain,
            Values::Int(checks),
        ));
    }
}

pub(crate) fn unseal(mesh: &mut Polymesh) {
    for domain in [Domain::Point, Domain::Edge, Domain::Face] {
        let checks = mesh
            .attributes
            .remove(check_name(domain))
            .filter(|attribute| attribute.domain == domain)
            .and_then(|attribute| match attribute.values {
                Values::Int(values) => Some(values),
                _ => None,
            });
        let Some(values) = mesh.attributes.values_mut::<i32>(name(domain), domain) else {
            continue;
        };
        match checks {
            Some(checks) if checks.len() == values.len() => {
                for (value, &check) in values.iter_mut().zip(&checks) {
                    if check_value(*value) != check {
                        *value = 0;
                    }
                }
            }
            _ => values.fill(0),
        }
    }
}

pub(crate) fn values(mesh: &Polymesh, domain: Domain) -> Option<Vec<i32>> {
    mesh.attributes
        .values::<i32>(name(domain), domain)
        .map(<[i32]>::to_vec)
}

pub(crate) fn assign(mesh: &mut Polymesh, domain: Domain, values: Vec<i32>) {
    if values.len() == mesh.domain_len(domain) {
        mesh.attributes
            .insert(Attribute::new(name(domain), domain, Values::Int(values)));
    }
}

pub(crate) fn clear(mesh: &mut Polymesh, domain: Domain, elements: std::ops::Range<usize>) {
    if let Some(values) = mesh.attributes.values_mut::<i32>(name(domain), domain) {
        let end = elements.end.min(values.len());
        values[elements.start.min(end)..end].fill(0);
    }
}

pub(crate) fn unify(mesh: &mut Polymesh, domain: Domain, targets: &[u32]) {
    let Some(values) = mesh.attributes.values_mut::<i32>(name(domain), domain) else {
        return;
    };
    let snapshot = values.to_vec();
    for (element, &target) in targets.iter().enumerate() {
        if let (Some(slot), Some(&value)) = (values.get_mut(element), snapshot.get(target as usize))
        {
            *slot = value;
        }
    }
}

pub(crate) fn mark_control_edges(mesh: &mut Polymesh, control: impl Fn(usize) -> bool) {
    let flags: Vec<bool> = (0..mesh.edge_count()).map(control).collect();
    mesh.attributes.insert(Attribute::new(
        CONTROL_EDGE,
        Domain::Edge,
        Values::Bool(flags),
    ));
}

pub(crate) fn clear_control_edges(mesh: &mut Polymesh) {
    mesh.attributes.remove(CONTROL_EDGE);
}

pub(crate) fn mark_control_range(mesh: &mut Polymesh, edges: std::ops::Range<usize>) {
    set_control_range(mesh, edges, true);
}

pub(crate) fn unmark_control_range(mesh: &mut Polymesh, edges: std::ops::Range<usize>) {
    set_control_range(mesh, edges, false);
}

fn set_control_range(mesh: &mut Polymesh, edges: std::ops::Range<usize>, value: bool) {
    if let Some(flags) = mesh
        .attributes
        .values_mut::<bool>(CONTROL_EDGE, Domain::Edge)
    {
        let end = edges.end.min(flags.len());
        flags[edges.start.min(end)..end].fill(value);
    }
}
