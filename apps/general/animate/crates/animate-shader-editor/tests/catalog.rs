use animate_shader::empty_material;
use animate_shader_editor::{CATEGORIES, CatalogLine, Dragged, catalog_lines};
use rikka_shader_graph::{Category, Entry, NodeKind, Op, Scope, ValueType};

use crate::support::library;

fn entries(lines: &[CatalogLine]) -> Vec<&Entry> {
    lines
        .iter()
        .filter_map(|line| match line {
            CatalogLine::Entry(entry) => Some(entry),
            CatalogLine::Heading(_) => None,
        })
        .collect()
}

#[test]
fn browsing_groups_by_category_and_searching_ranks_by_title() {
    let library = library();
    let graph = empty_material(&library).unwrap();
    let catalog = graph.catalog(&library, &Scope::Main);
    let total = catalog.len();

    let browsed = catalog_lines(catalog.clone(), "", None);
    assert_eq!(entries(&browsed).len(), total);
    let mut seen = Vec::new();
    for line in &browsed {
        if let CatalogLine::Heading(category) = line {
            assert!(!seen.contains(category), "{category:?} heads one group");
            seen.push(*category);
        }
    }
    let order = |category: &Category| CATEGORIES.iter().position(|known| known == category);
    assert!(
        seen.windows(2)
            .all(|pair| order(&pair[0]) < order(&pair[1]))
    );
    assert!(matches!(browsed.first(), Some(CatalogLine::Heading(_))));

    let searched = catalog_lines(catalog.clone(), "multiply", None);
    assert!(
        searched
            .iter()
            .all(|line| matches!(line, CatalogLine::Entry(_)))
    );
    assert_eq!(entries(&searched)[0].kind, NodeKind::Op(Op::Multiply));
    assert!(catalog_lines(catalog, "zzqx", None).is_empty());
}

#[test]
fn a_dropped_link_keeps_only_nodes_that_can_take_it() {
    let library = library();
    let graph = empty_material(&library).unwrap();
    let catalog = graph.catalog(&library, &Scope::Main);

    let shaders = catalog_lines(
        catalog.clone(),
        "",
        Some(Dragged::FromOutput(Some(ValueType::Shader))),
    );
    let kept = entries(&shaders);
    assert!(kept.iter().any(|entry| entry.kind == NodeKind::MixShader));
    assert!(
        kept.iter()
            .all(|entry| !matches!(entry.kind, NodeKind::Op(_)))
    );

    let colours = catalog_lines(
        catalog,
        "",
        Some(Dragged::FromInput(Some(ValueType::Float3))),
    );
    let kept = entries(&colours);
    assert!(
        kept.iter()
            .any(|entry| entry.kind == NodeKind::Op(Op::Multiply))
    );
    assert!(kept.iter().all(|entry| entry.kind != NodeKind::Output));
    assert!(kept.iter().all(|entry| {
        entry.category != Category::Shader
            || entry
                .ports
                .outputs
                .iter()
                .any(|port| port.ty != rikka_shader_graph::PortType::Value(ValueType::Shader))
    }));
}
