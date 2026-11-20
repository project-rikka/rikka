use animate_shader::Merge;
use animate_ui::{choice, glyph, heading, labelled};
use rikka_app::prelude::*;
use rikka_renderer_3d::{Bounds, Hint, Literal, ParameterKind, Role, TextureKind};
use rikka_shader_graph::{Graph, ParameterDecl};

use crate::state::Popup;
use crate::traits::{Desk, Reach, Workshop};
use crate::views::rows::number_fields;
use crate::views::wording;

const ADD: &str = "animate-shader-add-parameter";
const MENU_WIDTH: f32 = 200.0;
const RANGE: f32 = 70.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Shape {
    Float,
    Vec2,
    Vec3,
    Vec4,
    Color,
    ColorAlpha,
    Int,
    UInt,
    Bool,
    Enum,
    Texture,
}

impl Shape {
    pub(crate) const ALL: [Self; 11] = [
        Self::Float,
        Self::Vec2,
        Self::Vec3,
        Self::Vec4,
        Self::Color,
        Self::ColorAlpha,
        Self::Int,
        Self::UInt,
        Self::Bool,
        Self::Enum,
        Self::Texture,
    ];

    fn of(kind: &ParameterKind) -> Self {
        match kind {
            ParameterKind::Float => Self::Float,
            ParameterKind::Vec2 => Self::Vec2,
            ParameterKind::Vec3 => Self::Vec3,
            ParameterKind::Vec4 => Self::Vec4,
            ParameterKind::Color { alpha: false, .. } => Self::Color,
            ParameterKind::Color { alpha: true, .. } => Self::ColorAlpha,
            ParameterKind::Int => Self::Int,
            ParameterKind::UInt => Self::UInt,
            ParameterKind::Bool => Self::Bool,
            ParameterKind::Enum { .. } => Self::Enum,
            ParameterKind::Texture { .. } => Self::Texture,
        }
    }

    fn kind(self) -> ParameterKind {
        match self {
            Self::Float => ParameterKind::Float,
            Self::Vec2 => ParameterKind::Vec2,
            Self::Vec3 => ParameterKind::Vec3,
            Self::Vec4 => ParameterKind::Vec4,
            Self::Color => ParameterKind::Color {
                alpha: false,
                srgb: true,
            },
            Self::ColorAlpha => ParameterKind::Color {
                alpha: true,
                srgb: true,
            },
            Self::Int => ParameterKind::Int,
            Self::UInt => ParameterKind::UInt,
            Self::Bool => ParameterKind::Bool,
            Self::Enum => ParameterKind::Enum {
                options: vec!["first".to_owned(), "second".to_owned()],
            },
            Self::Texture => ParameterKind::Texture {
                kind: TextureKind::Colour,
                sampler: true,
            },
        }
    }

    fn default(self) -> Option<Literal> {
        Some(match self {
            Self::Float => Literal::Float(0.0),
            Self::Vec2 => Literal::Vec2([0.0; 2]),
            Self::Vec3 => Literal::Vec3([0.0; 3]),
            Self::Vec4 => Literal::Vec4([0.0; 4]),
            Self::Color => Literal::Vec3([1.0; 3]),
            Self::ColorAlpha => Literal::Vec4([1.0; 4]),
            Self::Int => Literal::Int(0),
            Self::UInt => Literal::UInt(0),
            Self::Bool => Literal::Bool(false),
            Self::Enum => Literal::Enum(0),
            Self::Texture => return None,
        })
    }
}

const ROLES: [Option<Role>; 12] = [
    None,
    Some(Role::Albedo),
    Some(Role::AlbedoTexture),
    Some(Role::AlbedoWeight),
    Some(Role::Metalness),
    Some(Role::MetalnessTexture),
    Some(Role::Transmission),
    Some(Role::Subsurface),
    Some(Role::Emission),
    Some(Role::EmissionTexture),
    Some(Role::EmissionLuminance),
    Some(Role::Displacement),
];

const HINTS: [Hint; 4] = [Hint::Angle, Hint::Logarithmic, Hint::Hidden, Hint::Advanced];

const TEXTURE_KINDS: [TextureKind; 4] = [
    TextureKind::Colour,
    TextureKind::Data,
    TextureKind::Normal,
    TextureKind::Ramp,
];

fn fresh_name(graph: &Graph) -> String {
    (1..)
        .map(|number| format!("parameter_{number}"))
        .find(|name| graph.parameter(name).is_none())
        .expect("some numbered name is free")
}

fn set(decl: ParameterDecl) -> impl Fn(&mut dyn Workshop) + Clone {
    move |studio: &mut dyn Workshop| {
        studio.edit_graph(None, |graph, library| {
            graph.set_parameter(library, decl.clone())
        });
    }
}

pub(crate) fn parameters(desk: Desk<'_>, graph: &Graph) -> Element {
    let open = desk.shader().popup == Some(Popup::AddParameter);
    let mut header = vec![
        container(heading(tr!("animate.shader.parameters")))
            .grow(1.0)
            .any(),
        glyph(MaterialSymbol::ADD)
            .ghost()
            .on_press(desk.act(|studio| studio.toggle_popup(Popup::AddParameter)))
            .tooltip(tr!("animate.shader.add_parameter"))
            .build()
            .keyed(ADD),
    ];
    if open {
        let name = fresh_name(graph);
        let items = Shape::ALL.map(|shape| {
            let name = name.clone();
            menu_item(wording::parameter_shape(shape)).on_press(desk.act(move |studio| {
                studio.close_popup();
                let mut decl = ParameterDecl::new(name.clone(), shape.kind());
                decl.default = shape.default();
                studio.edit_graph(None, |graph, library| {
                    graph.add_parameter(library, decl.clone())
                });
            }))
        });
        header.push(
            menu()
                .items(items)
                .width(MENU_WIDTH)
                .on_dismiss(desk.act(|studio| studio.close_popup()))
                .build()
                .keyed((ADD, "menu"))
                .anchored(Anchor::below(ADD)),
        );
    }
    let mut parts = vec![row_of(header).align_items(Align::Center).fill_width().any()];
    if graph.parameters().is_empty() {
        parts.push(
            text(tr!("animate.shader.no_parameters"))
                .caption()
                .muted()
                .any(),
        );
    }
    for parameter in graph.parameters() {
        parts.push(parameter_card(desk, parameter));
    }
    column_of(parts).gap(Space::SNUG).fill_width().any()
}

fn default_editor(reach: Reach, parameter: &ParameterDecl) -> Option<Element> {
    let decl = parameter.clone();
    let with = move |literal: Literal| ParameterDecl {
        default: Some(literal),
        ..decl.clone()
    };
    let numbers = |values: Vec<f32>, make: fn(&[f32]) -> Literal| {
        let with = with.clone();
        let current = values.clone();
        number_fields(reach, &values, 0.05, move |studio, index, value| {
            let mut values = current.clone();
            values[index] = value;
            set(with(make(&values)))(studio);
        })
    };
    Some(match (&parameter.kind, parameter.default) {
        (ParameterKind::Texture { .. }, _) => return None,
        (ParameterKind::Bool, default) => {
            let on = matches!(default, Some(Literal::Bool(true)));
            switch(on)
                .control_size(ControlSize::Small)
                .on_toggle(
                    reach.act_with(move |studio, on: bool| set(with(Literal::Bool(on)))(studio)),
                )
                .any()
        }
        (ParameterKind::Enum { options }, default) => {
            let chosen = match default {
                Some(Literal::Enum(index)) => Some(index as usize),
                _ => None,
            };
            combo(chosen)
                .options(options.clone())
                .on_select(reach.act_with(move |studio, index: usize| {
                    set(with(Literal::Enum(index as u32)))(studio)
                }))
                .focusless()
                .control_size(ControlSize::Small)
                .fill_width()
                .any()
        }
        (ParameterKind::Color { alpha, .. }, default) => {
            let values = match default {
                Some(Literal::Vec3([r, g, b])) => [r, g, b, 1.0],
                Some(Literal::Vec4(values)) => values,
                _ => [1.0; 4],
            };
            let alpha = *alpha;
            let merge = Merge(rikka_ui::hash_of(("parameter", &parameter.name)));
            color_well(Color::rgba(values[0], values[1], values[2], values[3]))
                .alpha(alpha)
                .control_size(ControlSize::Small)
                .on_change(reach.act_with(move |studio, color: Color| {
                    let literal = match alpha {
                        true => Literal::Vec4([color.r, color.g, color.b, color.a]),
                        false => Literal::Vec3([color.r, color.g, color.b]),
                    };
                    let decl = with(literal);
                    studio.edit_graph(Some(merge), |graph, library| {
                        graph.set_parameter(library, decl.clone())
                    });
                }))
                .on_commit(reach.act_with(move |studio, _: Color| studio.seal()))
                .any()
        }
        (ParameterKind::Float, default) => numbers(
            vec![match default {
                Some(Literal::Float(value)) => value,
                _ => 0.0,
            }],
            |values| Literal::Float(values[0]),
        ),
        (ParameterKind::Vec2, default) => numbers(
            match default {
                Some(Literal::Vec2(values)) => values.to_vec(),
                _ => vec![0.0; 2],
            },
            |values| Literal::Vec2([values[0], values[1]]),
        ),
        (ParameterKind::Vec3, default) => numbers(
            match default {
                Some(Literal::Vec3(values)) => values.to_vec(),
                _ => vec![0.0; 3],
            },
            |values| Literal::Vec3([values[0], values[1], values[2]]),
        ),
        (ParameterKind::Vec4, default) => numbers(
            match default {
                Some(Literal::Vec4(values)) => values.to_vec(),
                _ => vec![0.0; 4],
            },
            |values| Literal::Vec4([values[0], values[1], values[2], values[3]]),
        ),
        (ParameterKind::Int, default) => numbers(
            vec![match default {
                Some(Literal::Int(value)) => value as f32,
                _ => 0.0,
            }],
            |values| Literal::Int(values[0].round() as i32),
        ),
        (ParameterKind::UInt, default) => numbers(
            vec![match default {
                Some(Literal::UInt(value)) => value as f32,
                _ => 0.0,
            }],
            |values| Literal::UInt(values[0].max(0.0).round() as u32),
        ),
    })
}

fn range_editor(reach: Reach, parameter: &ParameterDecl) -> Option<Element> {
    if !matches!(
        parameter.kind,
        ParameterKind::Float
            | ParameterKind::Vec2
            | ParameterKind::Vec3
            | ParameterKind::Vec4
            | ParameterKind::Int
            | ParameterKind::UInt
    ) {
        return None;
    }
    let decl = parameter.clone();
    let toggle = switch(parameter.range.is_some())
        .control_size(ControlSize::Small)
        .on_toggle(reach.act_with(move |studio, on: bool| {
            set(ParameterDecl {
                range: on.then_some(Bounds {
                    low: 0.0,
                    high: 1.0,
                }),
                ..decl.clone()
            })(studio)
        }));
    let mut parts = vec![toggle.any()];
    if let Some(range) = parameter.range {
        for (index, value) in [range.low, range.high].into_iter().enumerate() {
            let decl = parameter.clone();
            parts.push(
                number_field(value)
                    .step(0.1)
                    .decimals(3)
                    .control_size(ControlSize::Small)
                    .on_commit(reach.act_with(move |studio, value: f32| {
                        let mut range = decl.range.unwrap_or(Bounds {
                            low: 0.0,
                            high: 1.0,
                        });
                        match index {
                            0 => range.low = value,
                            _ => range.high = value,
                        }
                        set(ParameterDecl {
                            range: Some(range),
                            ..decl.clone()
                        })(studio)
                    }))
                    .width(RANGE)
                    .any(),
            );
        }
    }
    Some(
        row_of(parts)
            .gap(Space::TIGHT)
            .align_items(Align::Center)
            .any(),
    )
}

fn text_setting(
    desk: Desk<'_>,
    parameter: &ParameterDecl,
    field: &'static str,
    current: Option<&str>,
    apply: fn(ParameterDecl, Option<String>) -> ParameterDecl,
) -> Element {
    let key = format!(
        "{:?}/parameter/{}/{field}",
        desk.shader().graph(),
        parameter.name
    );
    let document = desk.shader().field(&key, current.unwrap_or_default());
    let reading = document.clone();
    let decl = parameter.clone();
    let commit = desk.act(move |studio| {
        let text = reading.text().trim().to_owned();
        let value = (!text.is_empty()).then_some(text);
        set(apply(decl.clone(), value))(studio);
        reading.mark_saved();
    });
    text_field(&document)
        .control_size(ControlSize::Small)
        .on_submit(commit.clone())
        .on_blur(commit)
        .fill_width()
        .any()
}

fn small_combo(chosen: Option<usize>, options: Vec<String>, select: Handler<(usize,)>) -> Element {
    combo(chosen)
        .options(options)
        .on_select(select)
        .focusless()
        .control_size(ControlSize::Small)
        .fill_width()
        .any()
}

pub(crate) fn parameter_card(desk: Desk<'_>, parameter: &ParameterDecl) -> Element {
    let shape = Shape::of(&parameter.kind);
    let name_key = format!(
        "{:?}/parameter/{}/name",
        desk.shader().graph(),
        parameter.name
    );
    let naming = desk.shader().field(&name_key, &parameter.name);
    let reading = naming.clone();
    let from = parameter.name.clone();
    let rename = desk.act(move |studio| {
        let to = reading.text().trim().to_owned();
        if to != from {
            studio.edit_graph(None, |graph, library| {
                graph.rename_parameter(library, &from, &to)
            });
        }
        reading.mark_saved();
    });
    let decl = parameter.clone();
    let shape_combo = small_combo(
        Shape::ALL.iter().position(|each| *each == shape),
        Shape::ALL
            .map(|shape| wording::parameter_shape(shape).to_string())
            .to_vec(),
        desk.act_with(move |studio, index: usize| {
            let shape = Shape::ALL[index];
            set(ParameterDecl {
                kind: shape.kind(),
                default: shape.default(),
                range: None,
                soft_range: None,
                role: None,
                ..decl.clone()
            })(studio)
        }),
    );
    let decl = parameter.clone();
    let role = small_combo(
        ROLES.iter().position(|each| *each == parameter.role),
        ROLES.map(|role| wording::role(role).to_string()).to_vec(),
        desk.act_with(move |studio, index: usize| {
            set(ParameterDecl {
                role: ROLES[index],
                ..decl.clone()
            })(studio)
        }),
    );
    let hints = row_of(HINTS.map(|hint| {
        let on = parameter.hints.contains(&hint);
        let decl = parameter.clone();
        choice(wording::hint(hint), on)
            .on_press(desk.act(move |studio| {
                let mut hints = decl.hints.clone();
                match hints.contains(&hint) {
                    true => hints.retain(|each| *each != hint),
                    false => hints.push(hint),
                }
                set(ParameterDecl {
                    hints,
                    ..decl.clone()
                })(studio)
            }))
            .any()
    }))
    .gap(Space::TIGHT)
    .wrap_items();
    let texture_kind = match parameter.kind {
        ParameterKind::Texture { kind, sampler } => {
            let decl = parameter.clone();
            Some(small_combo(
                TEXTURE_KINDS.iter().position(|each| *each == kind),
                TEXTURE_KINDS.map(wording::texture_kind).to_vec(),
                desk.act_with(move |studio, index: usize| {
                    set(ParameterDecl {
                        kind: ParameterKind::Texture {
                            kind: TEXTURE_KINDS[index],
                            sampler,
                        },
                        ..decl.clone()
                    })(studio)
                }),
            ))
        }
        _ => None,
    };
    let removed = parameter.name.clone();
    let remove = glyph(MaterialSymbol::DELETE)
        .ghost()
        .on_press(desk.act(move |studio| {
            studio.edit_graph(None, |graph, library| {
                graph.remove_parameter(library, &removed)
            });
        }))
        .tooltip(tr!("animate.shader.remove_parameter"));
    let mut rows = vec![
        row_of([
            text_field(&naming)
                .control_size(ControlSize::Small)
                .on_submit(rename.clone())
                .on_blur(rename)
                .grow(1.0)
                .any(),
            remove,
        ])
        .gap(Space::TIGHT)
        .align_items(Align::Center)
        .fill_width()
        .any(),
        labelled(tr!("animate.shader.kind"), shape_combo, None),
    ];
    if let Some(texture_kind) = texture_kind {
        rows.push(labelled(
            tr!("animate.shader.texture_kind"),
            texture_kind,
            None,
        ));
    }
    if let Some(default) = default_editor(desk.reach(), parameter) {
        rows.push(labelled(tr!("animate.shader.default"), default, None));
    }
    if let Some(range) = range_editor(desk.reach(), parameter) {
        rows.push(labelled(tr!("animate.shader.range"), range, None));
    }
    rows.push(labelled(
        tr!("animate.shader.group_name"),
        text_setting(
            desk,
            parameter,
            "group",
            parameter.group.as_deref(),
            |decl, group| ParameterDecl { group, ..decl },
        ),
        None,
    ));
    rows.push(labelled(
        tr!("animate.shader.label"),
        text_setting(
            desk,
            parameter,
            "label",
            parameter.label.as_deref(),
            |decl, label| ParameterDecl { label, ..decl },
        ),
        None,
    ));
    rows.push(labelled(tr!("animate.shader.role"), role, None));
    if let ParameterKind::Enum { options } = &parameter.kind {
        rows.push(labelled(
            tr!("animate.shader.options"),
            text_setting(
                desk,
                parameter,
                "options",
                Some(&options.join("|")),
                |decl, text| ParameterDecl {
                    kind: ParameterKind::Enum {
                        options: text
                            .unwrap_or_default()
                            .split('|')
                            .map(|option| option.trim().to_owned())
                            .filter(|option| !option.is_empty())
                            .collect(),
                    },
                    ..decl
                },
            ),
            None,
        ));
    }
    rows.push(hints.any());
    container(column_of(rows).gap(Space::TIGHT).fill_width())
        .padding(Inset::all(8.0))
        .background(palette().surface_variant)
        .radius(Corner::Control)
        .fill_width()
        .any()
}
