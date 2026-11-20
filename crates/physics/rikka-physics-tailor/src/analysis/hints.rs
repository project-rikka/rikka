use rikka_model_core::NodeId;

use crate::figure::Figure;

pub(crate) fn carried(figure: &Figure<'_>) -> Vec<bool> {
    (0..figure.model.nodes.len())
        .map(|index| figure.hung(NodeId(index as u32)).is_some())
        .collect()
}
