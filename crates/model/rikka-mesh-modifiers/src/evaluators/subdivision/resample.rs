use rikka_polymesh::{AttributeValue, Domain, Polymesh, Values};

use super::stencil::Stencil;
use crate::parallel;

const GRAIN: usize = 1 << 14;

macro_rules! map_values {
    ($values:expr, $list:ident => $body:expr) => {
        match $values {
            Values::Bool($list) => Values::Bool($body),
            Values::Int($list) => Values::Int($body),
            Values::Float($list) => Values::Float($body),
            Values::Vec2($list) => Values::Vec2($body),
            Values::Vec3($list) => Values::Vec3($body),
            Values::Color($list) => Values::Color($body),
        }
    };
}

fn mixed<T: AttributeValue>(list: &[T], stencil: &Stencil) -> Vec<T> {
    let mut output = vec![T::DEFAULT; stencil.rows()];
    stencil.fill_rows(&mut output, |row| {
        T::mix(
            row.iter()
                .map(|&(source, weight)| (list[source as usize], weight)),
        )
    });
    output
}

fn halves<T: AttributeValue>(list: &[T], len: usize) -> Vec<T> {
    let mut output = vec![T::DEFAULT; len];
    let pairs = (list.len() * 2).min(len);
    parallel::fill(&mut output[..pairs], GRAIN, |index, slot| {
        *slot = list[index / 2]
    });
    output
}

fn repeated<T: AttributeValue>(list: &[T], owners: &[u32]) -> Vec<T> {
    parallel::collect(owners.len(), GRAIN, |index| list[owners[index] as usize])
}

pub(super) fn interpolate(mesh: &mut Polymesh, domain: Domain, stencil: &Stencil) {
    if domain == Domain::Point && mesh.vertex_groups.has_weights() {
        let groups = &mesh.vertex_groups;
        let mut weights = vec![Vec::new(); stencil.rows()];
        stencil.fill_rows(&mut weights, |row| groups.mixed(row));
        mesh.vertex_groups.weights = weights;
    }
    for attribute in mesh.attributes.iter_mut() {
        if attribute.domain == domain {
            attribute.values = map_values!(&attribute.values, list => mixed(list, stencil));
        }
    }
}

pub(super) fn split_edges(mesh: &mut Polymesh, len: usize) {
    for attribute in mesh.attributes.iter_mut() {
        if attribute.domain == Domain::Edge {
            attribute.values = map_values!(&attribute.values, list => halves(list, len));
        }
    }
}

pub(super) fn repeat_faces(mesh: &mut Polymesh, owners: &[u32]) {
    for attribute in mesh.attributes.iter_mut() {
        if attribute.domain == Domain::Face {
            attribute.values = map_values!(&attribute.values, list => repeated(list, owners));
        }
    }
}
