mod class;
mod machine;
mod rules;
mod unit;

use std::collections::BTreeSet;

pub(crate) use machine::{Action, Ahead, Machine};
pub(crate) use rules::Decision;

use crate::enumeration::Column;
use crate::error::Result;
use crate::model::Model;
use class::Class;
use unit::Unit;

const DOTTED_CIRCLE: usize = 0x25CC;

#[derive(Debug)]
pub(crate) struct LineBreaking {
    pub(crate) machine: Machine,
    pub(crate) columns: Vec<u8>,
}

impl LineBreaking {
    pub(crate) fn derive(model: &Model) -> Result<Self> {
        let short = |column: &Column, codepoint: usize| -> String {
            column.enumeration.variants[usize::from(column.values[codepoint])]
                .short
                .clone()
        };
        let mut classes = std::collections::HashMap::new();
        let kinds: Vec<Unit> = (0..crate::ucd::CODEPOINT_COUNT)
            .map(|codepoint| {
                let line_break = short(&model.line_break, codepoint);
                let category = short(&model.general_category, codepoint);
                let complex = line_break == "SA";
                let class = match classes.get(&(line_break.clone(), category.clone())) {
                    Some(&class) => class,
                    None => {
                        let class = Class::resolve(&line_break, &category)?;
                        classes.insert((line_break, category.clone()), class);
                        class
                    }
                };
                let width = short(&model.east_asian_width, codepoint);
                Ok(Unit {
                    complex,
                    class,
                    east_asian: matches!(width.as_str(), "F" | "W" | "H"),
                    initial_quote: category == "Pi",
                    final_quote: category == "Pf",
                    unassigned_pictographic: model.flags.extended_pictographic[codepoint]
                        && category == "Cn",
                    dotted_circle: codepoint == DOTTED_CIRCLE,
                    joiner_end: false,
                }
                .normalized())
            })
            .collect::<Result<_>>()?;
        let distinct: Vec<Unit> = kinds
            .iter()
            .copied()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        let machine = machine::build(&distinct);
        let columns = kinds
            .iter()
            .map(|kind| machine.kind_columns[kind])
            .collect();
        Ok(Self { machine, columns })
    }
}
