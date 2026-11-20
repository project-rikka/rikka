use animate_shader::Merge;
use animate_ui::{glyph, heading};
use rikka_app::prelude::*;
use rikka_shader_graph::{Curve, Interpolation, NodeId, NodeKind, Ramp, Scope, Stop};

use crate::traits::{Reach, Workshop};
use crate::views::rows::number_fields;
use crate::views::wording;

const STRIP: f32 = 22.0;
const PLOT: f32 = 140.0;
const COLUMNS: usize = 64;
const INTERPOLATIONS: [Interpolation; 3] = [
    Interpolation::Linear,
    Interpolation::Smooth,
    Interpolation::Constant,
];

fn shaped(t: f32, interpolation: Interpolation) -> f32 {
    match interpolation {
        Interpolation::Linear => t,
        Interpolation::Smooth => t * t * (3.0 - 2.0 * t),
        Interpolation::Constant => 0.0,
    }
}

fn sorted_stops(ramp: &Ramp) -> Vec<Stop> {
    let mut stops = ramp.stops.clone();
    stops.sort_by(|one, two| one.position.total_cmp(&two.position));
    stops
}

fn ramp_at(stops: &[Stop], interpolation: Interpolation, at: f32) -> [f32; 4] {
    let Some(first) = stops.first() else {
        return [0.0; 4];
    };
    if at <= first.position {
        return first.color;
    }
    for pair in stops.windows(2) {
        if at <= pair[1].position {
            let span = (pair[1].position - pair[0].position).max(1e-6);
            let t = shaped((at - pair[0].position) / span, interpolation);
            return std::array::from_fn(|channel| {
                pair[0].color[channel] + (pair[1].color[channel] - pair[0].color[channel]) * t
            });
        }
    }
    stops.last().map_or([0.0; 4], |stop| stop.color)
}

fn curve_at(points: &[[f32; 2]], interpolation: Interpolation, at: f32) -> f32 {
    let mut sorted = points.to_vec();
    sorted.sort_by(|one, two| one[0].total_cmp(&two[0]));
    let Some(first) = sorted.first() else {
        return 0.0;
    };
    if at <= first[0] {
        return first[1];
    }
    for pair in sorted.windows(2) {
        if at <= pair[1][0] {
            let span = (pair[1][0] - pair[0][0]).max(1e-6);
            let t = shaped((at - pair[0][0]) / span, interpolation);
            return pair[0][1] + (pair[1][1] - pair[0][1]) * t;
        }
    }
    sorted.last().map_or(0.0, |point| point[1])
}

#[derive(Debug)]
struct Strip {
    ramp: Ramp,
}

impl Widget for Strip {
    type State = ();

    fn layout(&self) -> Layout {
        Layout::row().size(Length::FILL, Length::px(STRIP)).fixed()
    }

    fn paint(&self, _state: &mut (), ctx: &mut PaintCtx<'_>) {
        let bounds = ctx.bounds();
        let stops = sorted_stops(&self.ramp);
        let width = bounds.width() / COLUMNS as f32;
        for column in 0..COLUMNS {
            let at = (column as f32 + 0.5) / COLUMNS as f32;
            let [r, g, b, a] = ramp_at(&stops, self.ramp.interpolation, at);
            ctx.draw_quad(
                Quad::new(Rect::new(
                    bounds.x() + column as f32 * width,
                    bounds.y(),
                    width + 0.5,
                    bounds.height(),
                ))
                .fill(Color::rgba(r, g, b, a)),
            );
        }
        ctx.draw_quad(Quad::new(bounds).stroke(1.0, palette().outline).radius(3.0));
    }

    fn same(&self, other: &Self) -> bool {
        self.ramp == other.ramp
    }
}

#[derive(Debug)]
struct Plot {
    curve: Curve,
}

impl Widget for Plot {
    type State = ();

    fn layout(&self) -> Layout {
        Layout::row().size(Length::FILL, Length::px(PLOT)).fixed()
    }

    fn paint(&self, _state: &mut (), ctx: &mut PaintCtx<'_>) {
        let palette = palette();
        let bounds = ctx.bounds();
        ctx.draw_quad(
            Quad::new(bounds)
                .fill(palette.surface_variant)
                .stroke(1.0, palette.outline)
                .radius(3.0),
        );
        let place = |x: f32, y: f32| {
            Point::new(
                bounds.x() + x.clamp(0.0, 1.0) * bounds.width(),
                bounds.max_y() - y.clamp(0.0, 1.0) * bounds.height(),
            )
        };
        let mut before = place(
            0.0,
            curve_at(&self.curve.points, self.curve.interpolation, 0.0),
        );
        for column in 1..=COLUMNS {
            let x = column as f32 / COLUMNS as f32;
            let after = place(x, curve_at(&self.curve.points, self.curve.interpolation, x));
            ctx.draw_segment(before, after, 2.0, palette.accent);
            before = after;
        }
        for point in &self.curve.points {
            ctx.draw_circle(place(point[0], point[1]), 8.0, palette.text);
        }
    }

    fn same(&self, other: &Self) -> bool {
        self.curve == other.curve
    }
}

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

fn interpolation_combo(
    reach: Reach,
    current: Interpolation,
    apply: impl Fn(&mut dyn Workshop, Interpolation) + 'static,
) -> Element {
    combo(INTERPOLATIONS.iter().position(|each| *each == current))
        .options(INTERPOLATIONS.map(wording::interpolation))
        .on_select(reach.act_with(move |studio, index: usize| apply(studio, INTERPOLATIONS[index])))
        .focusless()
        .control_size(ControlSize::Small)
        .fill_width()
        .any()
}

fn remove_glyph(press: Handler) -> Element {
    glyph(MaterialSymbol::CLOSE)
        .ghost()
        .on_press(press)
        .tooltip(tr!("animate.shader.remove"))
}

pub(crate) fn ramp_editor(reach: Reach, scope: &Scope, id: NodeId, ramp: &Ramp) -> Element {
    let mut parts = vec![
        heading(tr!("animate.shader.stops")),
        Element::new(Strip { ramp: ramp.clone() }),
    ];
    {
        let (scope, ramp) = (scope.clone(), ramp.clone());
        parts.push(interpolation_combo(
            reach,
            ramp.interpolation,
            move |studio, interpolation| {
                let ramp = Ramp {
                    interpolation,
                    ..ramp.clone()
                };
                set_kind(studio, &scope, id, NodeKind::ColorRamp(ramp), None);
            },
        ));
    }
    for (index, stop) in ramp.stops.iter().enumerate() {
        let position = {
            let (scope, ramp) = (scope.clone(), ramp.clone());
            number_fields(reach, &[stop.position], 0.05, move |studio, _, value| {
                let mut ramp = ramp.clone();
                ramp.stops[index].position = value.clamp(0.0, 1.0);
                set_kind(studio, &scope, id, NodeKind::ColorRamp(ramp), None);
            })
        };
        let well = {
            let (scope, ramp) = (scope.clone(), ramp.clone());
            let merge = Merge(rikka_ui::hash_of(("stop", id.get(), index)));
            let [r, g, b, a] = stop.color;
            color_well(Color::rgba(r, g, b, a))
                .alpha(true)
                .control_size(ControlSize::Small)
                .on_change(reach.act_with(move |studio, color: Color| {
                    let mut ramp = ramp.clone();
                    ramp.stops[index].color = [color.r, color.g, color.b, color.a];
                    set_kind(studio, &scope, id, NodeKind::ColorRamp(ramp), Some(merge));
                }))
                .on_commit(reach.act_with(move |studio, _: Color| studio.seal()))
                .any()
        };
        let remove = {
            let (scope, ramp) = (scope.clone(), ramp.clone());
            remove_glyph(reach.act(move |studio| {
                let mut ramp = ramp.clone();
                ramp.stops.remove(index);
                set_kind(studio, &scope, id, NodeKind::ColorRamp(ramp), None);
            }))
        };
        parts.push(
            row_of([position, well, remove])
                .gap(Space::TIGHT)
                .align_items(Align::Center)
                .any(),
        );
    }
    let (scope, adding) = (scope.clone(), ramp.clone());
    parts.push(
        button(text(tr!("animate.shader.add_stop")).caption())
            .ghost()
            .small()
            .focusless()
            .on_press(reach.act(move |studio| {
                let mut ramp = adding.clone();
                let stops = sorted_stops(&ramp);
                let color = ramp_at(&stops, ramp.interpolation, 0.5);
                ramp.stops.push(Stop {
                    position: 0.5,
                    color,
                });
                set_kind(studio, &scope, id, NodeKind::ColorRamp(ramp), None);
            }))
            .any(),
    );
    column_of(parts).gap(Space::TIGHT).fill_width().any()
}

pub(crate) fn curve_editor(reach: Reach, scope: &Scope, id: NodeId, curve: &Curve) -> Element {
    let mut parts = vec![
        heading(tr!("animate.shader.points")),
        Element::new(Plot {
            curve: curve.clone(),
        }),
    ];
    {
        let (scope, curve) = (scope.clone(), curve.clone());
        parts.push(interpolation_combo(
            reach,
            curve.interpolation,
            move |studio, interpolation| {
                let curve = Curve {
                    interpolation,
                    ..curve.clone()
                };
                set_kind(studio, &scope, id, NodeKind::Curve(curve), None);
            },
        ));
    }
    for (index, point) in curve.points.iter().enumerate() {
        let fields = {
            let (scope, curve) = (scope.clone(), curve.clone());
            number_fields(reach, point, 0.05, move |studio, axis, value| {
                let mut curve = curve.clone();
                curve.points[index][axis] = value;
                set_kind(studio, &scope, id, NodeKind::Curve(curve), None);
            })
        };
        let remove = {
            let (scope, curve) = (scope.clone(), curve.clone());
            remove_glyph(reach.act(move |studio| {
                let mut curve = curve.clone();
                curve.points.remove(index);
                set_kind(studio, &scope, id, NodeKind::Curve(curve), None);
            }))
        };
        parts.push(
            row_of([fields, remove])
                .gap(Space::TIGHT)
                .align_items(Align::Center)
                .any(),
        );
    }
    let (scope, adding) = (scope.clone(), curve.clone());
    parts.push(
        button(text(tr!("animate.shader.add_point")).caption())
            .ghost()
            .small()
            .focusless()
            .on_press(reach.act(move |studio| {
                let mut curve = adding.clone();
                let y = curve_at(&curve.points, curve.interpolation, 0.5);
                curve.points.push([0.5, y]);
                set_kind(studio, &scope, id, NodeKind::Curve(curve), None);
            }))
            .any(),
    );
    column_of(parts).gap(Space::TIGHT).fill_width().any()
}
