use rikka_ime::{DEFAULT_LAYOUT, Group, InputSource, Member};

const KEYBOARD_ENGINE_PREFIX: &str = "keyboard-";

const VARIANT_SEPARATOR: char = '-';

pub fn sources_from_group(group: &Group) -> Vec<InputSource> {
    group
        .members
        .iter()
        .map(|member| source_from_member(group, member))
        .collect()
}

pub fn group_from_sources(group: &Group, sources: &[InputSource]) -> Group {
    let default_layout = match group.default_layout.is_empty() {
        false => group.default_layout.clone(),
        true => sources
            .first()
            .map_or_else(|| DEFAULT_LAYOUT.to_owned(), layout_name),
    };
    Group {
        name: group.name.clone(),
        default_layout,
        members: sources.iter().map(member_from_source).collect(),
    }
}

fn source_from_member(group: &Group, member: &Member) -> InputSource {
    let plain = member.engine.strip_prefix(KEYBOARD_ENGINE_PREFIX);
    let named = match (member.layout.as_str(), plain) {
        ("", Some(layout)) => layout,
        ("", None) => default_layout(group),
        (layout, _) => layout,
    };
    let (layout, variant) = named.split_once(VARIANT_SEPARATOR).unwrap_or((named, ""));
    let source = match layout.is_empty() {
        true => InputSource::layout(DEFAULT_LAYOUT),
        false => InputSource::layout(layout),
    }
    .variant(variant);
    match plain {
        Some(_) => source,
        None => source.engine(member.engine.clone()),
    }
}

fn member_from_source(source: &InputSource) -> Member {
    let layout = layout_name(source);
    let engine = match source.engine.is_empty() {
        true => format!("{KEYBOARD_ENGINE_PREFIX}{layout}"),
        false => source.engine.clone(),
    };
    Member::new(engine).with_layout(layout)
}

fn layout_name(source: &InputSource) -> String {
    match source.variant.is_empty() {
        true => source.layout.clone(),
        false => format!("{}{VARIANT_SEPARATOR}{}", source.layout, source.variant),
    }
}

fn default_layout(group: &Group) -> &str {
    match group.default_layout.is_empty() {
        true => DEFAULT_LAYOUT,
        false => &group.default_layout,
    }
}
