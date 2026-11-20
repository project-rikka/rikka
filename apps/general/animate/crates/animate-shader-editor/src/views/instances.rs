use animate_look::{Name, SlotKey, Target};
use animate_ui::heading;
use rikka_app::prelude::*;
use rikka_shader_graph::Graph;

use crate::sockets::parameter_rows;
use crate::traits::Desk;
use crate::views::rows::{ValueRow, value_row};

pub(crate) fn instances(desk: Desk<'_>, key: &str, graph: &Graph) -> Option<Element> {
    let (figure, chosen) = desk.chosen_figure_slots()?;
    let [material] = chosen.as_slice() else {
        return None;
    };
    let slot = desk.look().slots.slot(figure.name, *material)?;
    if slot.material.as_deref() != Some(key) {
        return None;
    }
    let title = figure
        .materials
        .iter()
        .find(|held| held.index == *material)
        .map_or_else(|| material.to_string(), |held| held.name.clone());
    let mut parts = vec![heading(tr!(
        "animate.shader.slot_values",
        slot = title.as_str()
    ))];
    for row in parameter_rows(graph) {
        let target = Target::Slot(SlotKey {
            figure: figure.name,
            material: *material,
            parameter: Name::new(&row.name),
        });
        let held = desk.value(&target);
        let keying = desk.key_state(&target);
        parts.push(value_row(
            desk.reach(),
            ValueRow {
                set: held.is_some(),
                value: held.or(row.default),
                label: Label::from(row.label),
                field: row.field,
                key: target,
                document: None,
            },
            keying,
        ));
    }
    Some(column_of(parts).gap(Space::SNUG).fill_width().any())
}
