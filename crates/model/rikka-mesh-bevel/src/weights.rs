use rikka_mesh_edit::EditMesh;
use rikka_polymesh::{DataType, Domain, Values};

#[derive(Clone, Debug, Default)]
pub(crate) struct WeightTable {
    values: Option<Vec<f32>>,
}

impl WeightTable {
    pub(crate) fn resolve(mesh: &EditMesh, name: &str, domain: Domain) -> WeightTable {
        if let Some(values) = mesh.attribute::<f32>(name, domain) {
            return WeightTable {
                values: Some(values.to_vec()),
            };
        }
        let Some(source_domain) = mesh.attribute_domain(name) else {
            return WeightTable::default();
        };
        let Some(source) = mesh
            .attributes(source_domain)
            .find(|attribute| attribute.name == name)
        else {
            return WeightTable::default();
        };
        let polymesh = mesh.to_polymesh();
        let Some(compact) = polymesh
            .attributes
            .get(name)
            .map(|attribute| attribute.values.convert(DataType::Float))
        else {
            return WeightTable::default();
        };
        let adapted = match source.domain == domain {
            true => compact,
            false => polymesh.adapt_domain(&compact, source.domain, domain),
        };
        let Values::Float(compact_values) = adapted else {
            return WeightTable::default();
        };
        let mut values = vec![0.0f32; mesh_capacity(mesh, domain)];
        let slots: Vec<usize> = match domain {
            Domain::Point => mesh.verts().map(|vert| vert.index()).collect(),
            Domain::Edge => mesh.edges().map(|edge| edge.index()).collect(),
            Domain::Face => mesh.faces().map(|face| face.index()).collect(),
            Domain::Corner => mesh
                .faces()
                .flat_map(|face| mesh.face_loops(face))
                .map(|lp| lp.index())
                .collect(),
        };
        for (slot, value) in slots.into_iter().zip(compact_values) {
            values[slot] = value;
        }
        WeightTable {
            values: Some(values),
        }
    }

    pub(crate) fn get(&self, index: usize) -> f32 {
        self.values
            .as_ref()
            .and_then(|values| values.get(index).copied())
            .unwrap_or(0.0)
    }
}

fn mesh_capacity(mesh: &EditMesh, domain: Domain) -> usize {
    match domain {
        Domain::Point => mesh.vert_capacity(),
        Domain::Edge => mesh.edge_capacity(),
        Domain::Face => mesh.face_capacity(),
        Domain::Corner => mesh.loop_capacity(),
    }
}
