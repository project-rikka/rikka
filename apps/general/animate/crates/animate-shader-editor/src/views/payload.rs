use animate_shader::Merge;
use animate_ui::labelled;
use rikka_app::prelude::*;
use rikka_renderer_3d::TextureKind;
use rikka_shader_graph::{
    BlendMode, Constant, Graph, Input, Layer, Library, NodeId, NodeKind, Noise, Op, Scope,
};

use crate::traits::{Desk, Reach, Workshop};
use crate::views::rows::number_fields;
use crate::views::{parameters, ramp, wording};

const TEXTURE_KINDS: [TextureKind; 4] = [
    TextureKind::Colour,
    TextureKind::Data,
    TextureKind::Normal,
    TextureKind::Ramp,
];

fn set_kind(
    studio: &mut dyn Workshop,
    scope: &Scope,
    id: NodeId,
    kind: NodeKind,
    merge: Option<Merge>,
) {
    studio.edit_graph(merge, |graph, library| {
        graph
            .edit(library, scope.clone())
            .set_kind(id, kind.clone())
    });
}

fn kind_combo<T: Clone + PartialEq + 'static>(
    reach: Reach,
    choices: Vec<T>,
    current: &T,
    label: impl Fn(&T) -> String,
    scope: &Scope,
    id: NodeId,
    kind: impl Fn(T) -> NodeKind + 'static,
) -> Element {
    let chosen = choices.iter().position(|choice| choice == current);
    let names: Vec<String> = choices.iter().map(label).collect();
    let scope = scope.clone();
    combo(chosen)
        .options(names)
        .on_select(reach.act_with(move |studio, index: usize| {
            if let Some(choice) = choices.get(index).cloned() {
                set_kind(studio, &scope, id, kind(choice), None);
            }
        }))
        .focusless()
        .control_size(ControlSize::Small)
        .fill_width()
        .any()
}

fn color_of(values: &[f32]) -> Color {
    Color::rgba(
        values.first().copied().unwrap_or(0.0),
        values.get(1).copied().unwrap_or(0.0),
        values.get(2).copied().unwrap_or(0.0),
        values.get(3).copied().unwrap_or(1.0),
    )
}

fn constant_editor(reach: Reach, scope: &Scope, id: NodeId, constant: &Constant) -> Vec<Element> {
    let values = constant.values().to_vec();
    let current = values.clone();
    let fields = {
        let scope = scope.clone();
        number_fields(reach, &values, 0.05, move |studio, index, value| {
            let mut values = current.clone();
            values[index] = value;
            if let Some(constant) = Constant::from_slice(&values) {
                set_kind(studio, &scope, id, NodeKind::Constant(constant), None);
            }
        })
    };
    let mut parts = vec![labelled(tr!("animate.shader.value"), fields, None)];
    if values.len() >= 3 {
        let width = values.len();
        let merge = Merge(rikka_ui::hash_of(("constant", id.get())));
        let scope = scope.clone();
        parts.push(labelled(
            tr!("animate.shader.colour"),
            color_well(color_of(&values))
                .alpha(width == 4)
                .control_size(ControlSize::Small)
                .on_change(reach.act_with(move |studio, color: Color| {
                    let values = [color.r, color.g, color.b, color.a];
                    if let Some(constant) = Constant::from_slice(&values[..width]) {
                        set_kind(
                            studio,
                            &scope,
                            id,
                            NodeKind::Constant(constant),
                            Some(merge),
                        );
                    }
                }))
                .on_commit(reach.act_with(|studio, _: Color| studio.seal()))
                .any(),
            None,
        ));
    }
    parts
}

fn mask_editor(desk: Desk<'_>, scope: &Scope, id: NodeId, mask: &str) -> Element {
    let key = format!("{:?}/{scope:?}/swizzle/{id}", desk.shader().graph());
    let document = desk.shader().field(&key, mask);
    let reading = document.clone();
    let scope = scope.clone();
    let commit = desk.act(move |studio| {
        let mask = reading.text().trim().to_owned();
        set_kind(studio, &scope, id, NodeKind::Swizzle(mask), None);
        reading.mark_saved();
    });
    labelled(
        tr!("animate.shader.mask"),
        text_field(&document)
            .control_size(ControlSize::Small)
            .on_submit(commit.clone())
            .on_blur(commit)
            .fill_width()
            .any(),
        None,
    )
}

fn call_editor(
    desk: Desk<'_>,
    graph: &Graph,
    scope: &Scope,
    id: NodeId,
    name: &str,
) -> Vec<Element> {
    let names: Vec<String> = graph
        .functions()
        .keys()
        .filter(|known| *scope != Scope::Function((*known).clone()))
        .cloned()
        .collect();
    let entered = name.to_owned();
    vec![
        labelled(
            tr!("animate.shader.called_group"),
            kind_combo(
                desk.reach(),
                names,
                &name.to_owned(),
                Clone::clone,
                scope,
                id,
                NodeKind::Call,
            ),
            None,
        ),
        row_of([
            button(text(tr!("animate.shader.enter_group")).caption())
                .small()
                .ghost()
                .focusless()
                .on_press(desk.act(move |studio| studio.enter(&entered)))
                .any(),
            button(text(tr!("animate.shader.ungroup")).caption())
                .small()
                .ghost()
                .focusless()
                .on_press(desk.act(move |studio| studio.expand_node(id)))
                .any(),
        ])
        .gap(Space::TIGHT)
        .any(),
    ]
}

pub(crate) fn kind_editor(
    desk: Desk<'_>,
    graph: &Graph,
    library: &Library,
    scope: &Scope,
    id: NodeId,
    kind: &NodeKind,
) -> Vec<Element> {
    let reach = desk.reach();
    let title = |kind: NodeKind| kind.title(library);
    let row = |label: Label, control: Element| vec![labelled(label, control, None)];
    match kind {
        NodeKind::Constant(constant) => constant_editor(reach, scope, id, constant),
        NodeKind::Parameter(name) => graph
            .parameter(name)
            .map(|parameter| vec![parameters::parameter_card(desk, parameter)])
            .unwrap_or_default(),
        NodeKind::Swizzle(mask) => vec![mask_editor(desk, scope, id, mask)],
        NodeKind::Texture(texture) => row(
            tr!("animate.shader.texture_kind"),
            kind_combo(
                reach,
                TEXTURE_KINDS.to_vec(),
                texture,
                |kind| wording::texture_kind(*kind),
                scope,
                id,
                NodeKind::Texture,
            ),
        ),
        NodeKind::Fbm(noise) => row(
            tr!("animate.shader.noise"),
            kind_combo(
                reach,
                vec![Noise::Value, Noise::Gradient],
                noise,
                |noise| wording::noise(*noise),
                scope,
                id,
                NodeKind::Fbm,
            ),
        ),
        NodeKind::Blend(mode) => row(
            tr!("animate.shader.blend"),
            kind_combo(
                reach,
                BlendMode::ALL.to_vec(),
                mode,
                |mode| title(NodeKind::Blend(*mode)),
                scope,
                id,
                NodeKind::Blend,
            ),
        ),
        NodeKind::Layer(layer) => row(
            tr!("animate.shader.layer"),
            kind_combo(
                reach,
                Layer::ALL.to_vec(),
                layer,
                |layer| title(NodeKind::Layer(*layer)),
                scope,
                id,
                NodeKind::Layer,
            ),
        ),
        NodeKind::Input(input) => row(
            tr!("animate.shader.input"),
            kind_combo(
                reach,
                Input::ALL.to_vec(),
                input,
                |input| title(NodeKind::Input(*input)),
                scope,
                id,
                NodeKind::Input,
            ),
        ),
        NodeKind::Op(op) => {
            let category = NodeKind::Op(*op).category(library);
            let ops: Vec<Op> = Op::ALL
                .into_iter()
                .filter(|other| NodeKind::Op(*other).category(library) == category)
                .collect();
            row(
                tr!("animate.shader.operation"),
                kind_combo(
                    reach,
                    ops,
                    op,
                    |op| title(NodeKind::Op(*op)),
                    scope,
                    id,
                    NodeKind::Op,
                ),
            )
        }
        NodeKind::Call(name) => call_editor(desk, graph, scope, id, name),
        NodeKind::ColorRamp(value) => vec![ramp::ramp_editor(reach, scope, id, value)],
        NodeKind::Curve(curve) => vec![ramp::curve_editor(reach, scope, id, curve)],
        NodeKind::Lighting
        | NodeKind::Emission
        | NodeKind::Transparent
        | NodeKind::MixShader
        | NodeKind::AddShader
        | NodeKind::ShaderToRgb
        | NodeKind::Sample
        | NodeKind::Extern(_)
        | NodeKind::FunctionInput(_)
        | NodeKind::FunctionOutput(_)
        | NodeKind::Output => Vec::new(),
    }
}
