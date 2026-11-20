use rikka_fuzzy::{Haystack, Needle, Score};
use rikka_shader_graph::{Category, Entry, Port, PortType, ValueType};

pub const CATEGORIES: [Category; 26] = [
    Category::Input,
    Category::Parameter,
    Category::Constant,
    Category::Shader,
    Category::Toon,
    Category::Lighting,
    Category::Surface,
    Category::Texture,
    Category::Color,
    Category::Ramp,
    Category::Noise,
    Category::Math,
    Category::Interpolation,
    Category::Trigonometry,
    Category::Vector,
    Category::Uv,
    Category::Layer,
    Category::Filter,
    Category::Lens,
    Category::Stylize,
    Category::Environment,
    Category::Light,
    Category::Render,
    Category::Stage,
    Category::Function,
    Category::Output,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dragged {
    FromOutput(Option<ValueType>),
    FromInput(Option<ValueType>),
}

#[derive(Clone, Debug, PartialEq)]
pub enum CatalogLine {
    Heading(Category),
    Entry(Entry),
}

pub fn accepts(from: Option<ValueType>, port: &Port) -> bool {
    let Some(from) = from else {
        return true;
    };
    match port.ty {
        PortType::Dynamic => from.is_number() || from == ValueType::Image,
        PortType::Value(to) => match (from.width(), to.width()) {
            _ if from == to => true,
            (Some(1), Some(_)) => true,
            (Some(from), Some(to)) => from > to,
            (Some(_), None) => to == ValueType::Image,
            (None, Some(_)) => from == ValueType::Image,
            (None, None) => false,
        },
    }
}

pub fn produces(port: &Port, to: Option<ValueType>) -> bool {
    let Some(to) = to else {
        return true;
    };
    let target = Port {
        ty: PortType::Value(to),
        ..port.clone()
    };
    match port.ty {
        PortType::Dynamic => to.is_number() || to == ValueType::Image,
        PortType::Value(from) => accepts(Some(from), &target),
    }
}

fn matches_drag(entry: &Entry, dragged: Option<Dragged>) -> bool {
    match dragged {
        None => true,
        Some(Dragged::FromOutput(ty)) => entry.ports.inputs.iter().any(|port| accepts(ty, port)),
        Some(Dragged::FromInput(ty)) => entry.ports.outputs.iter().any(|port| produces(port, ty)),
    }
}

fn order(category: Category) -> usize {
    CATEGORIES
        .iter()
        .position(|known| *known == category)
        .unwrap_or(CATEGORIES.len())
}

pub fn catalog_lines(
    entries: Vec<Entry>,
    query: &str,
    dragged: Option<Dragged>,
) -> Vec<CatalogLine> {
    let mut kept: Vec<Entry> = entries
        .into_iter()
        .filter(|entry| matches_drag(entry, dragged))
        .collect();
    let needle = Needle::new(query.trim());
    if !needle.is_empty() {
        let mut scored: Vec<(Score, usize, Entry)> = kept
            .into_iter()
            .filter_map(|entry| {
                let score = needle.find(&Haystack::new(&entry.title))?;
                Some((score, order(entry.category), entry))
            })
            .collect();
        scored.sort_by(|one, two| two.0.cmp(&one.0).then(one.1.cmp(&two.1)));
        return scored
            .into_iter()
            .map(|(_, _, entry)| CatalogLine::Entry(entry))
            .collect();
    }
    kept.sort_by_key(|entry| order(entry.category));
    let mut lines = Vec::with_capacity(kept.len() + CATEGORIES.len());
    let mut last = None;
    for entry in kept {
        if last != Some(entry.category) {
            last = Some(entry.category);
            lines.push(CatalogLine::Heading(entry.category));
        }
        lines.push(CatalogLine::Entry(entry));
    }
    lines
}
