use rikka_polymesh::Polymesh;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Group {
    Unset,
    Unweighted,
    Weighted(u32),
}

impl Group {
    pub(crate) fn find(mesh: &Polymesh, name: &str) -> Group {
        if name.is_empty() {
            return Group::Unset;
        }
        match mesh.vertex_groups.index(name) {
            None => Group::Unset,
            Some(_) if !mesh.vertex_groups.has_weights() => Group::Unweighted,
            Some(group) => Group::Weighted(group),
        }
    }

    pub(crate) fn weight(self, mesh: &Polymesh, point: usize, invert: bool) -> f32 {
        let weight = match self {
            Group::Weighted(group) => mesh.vertex_groups.weight(point, group),
            Group::Unset | Group::Unweighted => return 1.0,
        };
        match invert {
            true => 1.0 - weight,
            false => weight,
        }
    }

    pub(crate) fn weight_or_zero(self, mesh: &Polymesh, point: usize, invert: bool) -> f32 {
        match self {
            Group::Weighted(group) => {
                let weight = mesh.vertex_groups.weight(point, group);
                match invert {
                    true => 1.0 - weight,
                    false => weight,
                }
            }
            Group::Unweighted => match invert {
                true => 1.0,
                false => 0.0,
            },
            Group::Unset => 1.0,
        }
    }
}
