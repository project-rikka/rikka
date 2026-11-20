use rikka_text_catalog::{FontStyle, Query, Weight};
use rikka_text_font::Tag;

use crate::support::{family, file_name, matched, query, session};

fn style_of(query: &Query) -> String {
    session()
        .face(matched(query).face())
        .style_name()
        .to_owned()
}

fn variation(query: &Query, tag: &[u8; 4], size: f32) -> Option<f32> {
    matched(query)
        .variations(size)
        .find(|&(axis, _)| axis == Tag::new(tag))
        .map(|(_, value)| value)
}

#[test]
fn static_weights_follow_css_matching() {
    let inter = query(&["Inter"]);
    for (weight, style) in [
        (300.0, "Light"),
        (400.0, "Regular"),
        (500.0, "Medium"),
        (700.0, "Bold"),
        (450.0, "Medium"),
        (350.0, "Light"),
        (550.0, "SemiBold"),
        (1000.0, "Black"),
        (1.0, "Thin"),
    ] {
        let found = style_of(&inter.clone().weight(Weight(weight)));
        assert_eq!(found, style, "weight {weight}");
    }
    assert!(
        session()
            .query(&inter.clone().weight(Weight::BOLD))
            .is_some_and(|found| !found.synthesis().bold)
    );
}

#[test]
fn variable_weights_match_axis_exactly() {
    let variable = query(&["Inter Variable"]);
    for weight in [300.0, 400.0, 500.0, 700.0, 850.0] {
        let request = variable.clone().weight(Weight(weight));
        let found = matched(&request);
        assert_eq!(file_name(found.face()), "InterVariable.ttf");
        assert_eq!(variation(&request, b"wght", 16.0), Some(weight));
        assert!(!found.synthesis().bold && !found.synthesis().oblique);
    }
    assert_eq!(
        variation(&variable.clone().weight(Weight(1000.0)), b"wght", 16.0),
        Some(900.0)
    );
}

#[test]
fn missing_styles_are_synthesized() {
    let italic = matched(&query(&["Noto Sans Tifinagh"]).style(FontStyle::Italic));
    assert!(italic.synthesis().oblique);
    let bold = matched(&query(&["Noto Sans Tifinagh"]).weight(Weight::BOLD));
    assert!(bold.synthesis().bold);
    let upright = matched(&query(&["Inter"]).style(FontStyle::Normal));
    assert!(!upright.synthesis().oblique && !upright.synthesis().bold);
}

#[test]
fn unknown_families_fall_through() {
    let found = matched(&query(&["No Such Family", "JetBrains Mono"]));
    assert_eq!(family(found.face()), "JetBrains Mono");
    let fallback = matched(&query(&["No Such Family"]));
    assert_eq!(family(fallback.face()), "Inter");
}
