use paint_document::{Combine, FloodMode, StackOrder};
use rikka_app::prelude::*;
use rikka_figure::Lettering;
use rikka_figure_editing::{align_choice, font_choice};
use rikka_image_edit::GradientShape;
use rikka_widgets::row;

use super::{
    BOX_WIDEST, Brushing, Filling, Flooding, GradientMode, Gradienting, Letters, Options,
    SELECTING, SampleFrom, SampleSize, Sampling, Setting, THICKEST, THINNEST,
};
use crate::camera::Camera;
use crate::command::{Command, Objecting};
use crate::paint::Paint;
use crate::tab::Tab;
use crate::tool::Tool;
use crate::wording;

const GLYPH: f32 = 18.0;
const SLIDER: f32 = 160.0;
const FIELD: f32 = 84.0;
const ZOOMS: f32 = 104.0;
const CHIP: f32 = 16.0;
const SHARE: f32 = 72.0;
const HEIGHT: f32 = 40.0;
const LEAD: f32 = 32.0;
const PRESETS: [f32; 8] = [0.25, 0.5, 1.0, 2.0, 4.0, 8.0, 16.0, 32.0];
const FONT: f32 = 200.0;
const STYLE_TILE: f32 = 28.0;
const STYLE_GLYPH: f32 = 18.0;
const ACTIVE_TINT: f32 = 0.16;
const SELECTION_MODES: f32 = 112.0;
const GRADIENT_SHAPES: f32 = 156.0;
const GRADIENT_MODES: f32 = 128.0;
const FILLINGS: f32 = 152.0;
const SOURCES: f32 = 96.0;
const SAMPLE_SIZES: f32 = 168.0;
const ALIGNMENTS: f32 = 108.0;

const ARRANGING: [(Command, MaterialSymbol, &str); 6] = [
    (
        Command::Object(Objecting::Stack(StackOrder::Front)),
        MaterialSymbol::FLIP_TO_FRONT,
        "paint.command.bring_to_front",
    ),
    (
        Command::Object(Objecting::Stack(StackOrder::Forward)),
        MaterialSymbol::ARROW_UPWARD,
        "paint.command.bring_forward",
    ),
    (
        Command::Object(Objecting::Stack(StackOrder::Backward)),
        MaterialSymbol::ARROW_DOWNWARD,
        "paint.command.send_backward",
    ),
    (
        Command::Object(Objecting::Stack(StackOrder::Back)),
        MaterialSymbol::FLIP_TO_BACK,
        "paint.command.send_to_back",
    ),
    (
        Command::Object(Objecting::Duplicate),
        MaterialSymbol::CONTENT_COPY,
        "paint.command.duplicate",
    ),
    (
        Command::Object(Objecting::Delete),
        MaterialSymbol::DELETE,
        "paint.command.delete",
    ),
];

pub(crate) fn bar(paint: &Paint, tab: &Tab) -> impl View {
    let tool = paint.tool();
    let styled = paint.styled_tool();
    let options = paint.options();
    let lead = row! {
        row! {
            icon(tool.symbol())
                .extent(Length::px(GLYPH))
                .color(palette().accent);
            text(tool.name()).caption().bold().no_wrap();
        }
        .gap(Space::TIGHT)
        .align_items(Align::Center);
        rule_upright();
    }
    .gap(Space::NORMAL)
    .align_items(Align::Center)
    .height(LEAD);
    let items = match styled {
        Tool::Paintbrush | Tool::Eraser => brushing(lead, styled, options),
        Tool::Gradient => gradienting(lead, options.gradient),
        Tool::FreeformShape => freeform(lead, options),
        _ if styled.sized() => drawing(lead, styled, options),
        Tool::Objects => row! {
            lead;
        },
        Tool::RectangleSelect => row! {
            lead;
            selecting(options.selecting);
        },
        Tool::EllipseSelect | Tool::Lasso | Tool::PolygonLasso => row! {
            lead;
            selecting(options.selecting);
            antialiasing(options.smoothing, Paint::set_smoothing);
        },
        Tool::MagicWand => flooding(lead, options.wand, Some(options.selecting)),
        Tool::PaintBucket => flooding(lead, options.bucket, None),
        Tool::Text => lettering(lead, &options.letters),
        Tool::ColorPicker => sampling(lead, options.sampling, paint.last_sample()),
        _ => row! {
            lead;
            zooming(tab.camera().clone());
        },
    };
    let items = match tool.picks() {
        true => items.push(arranging(paint)),
        false => items,
    };
    container(
        items
            .gap(Space::ROOMY)
            .align_items(Align::Center)
            .wrap_items()
            .padding(Inset::xy(Space::NORMAL, Space::TIGHT))
            .min_height(HEIGHT)
            .fill_width(),
    )
    .background(palette().surface)
    .fill_width()
}

fn labelled(label: Label, control: impl View) -> impl View {
    row! {
        text(label).caption().muted().no_wrap();
        control;
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
}

fn arranging(paint: &Paint) -> impl View {
    let buttons: Vec<Element> = ARRANGING
        .into_iter()
        .map(|(command, symbol, label)| {
            button(icon(symbol))
                .ghost()
                .small()
                .icon_only()
                .focusless()
                .enabled(command.enabled(paint))
                .on_press(move |paint: &mut Paint| command.run(paint))
                .tooltip(command.tooltip(tr!(label)))
        })
        .collect();
    row_of(buttons).gap(Space::HAIR).align_items(Align::Center)
}

fn drawing(lead: impl View, tool: Tool, options: &Options) -> Flex {
    let fills = tool.fills();
    let width = options.width(tool).unwrap_or_default();
    row! {
        lead;
        brush(width, !fills || options.filling.outlines());
        if fills {
            filled(options.filling);
        }
    }
}

fn brushing(lead: impl View, tool: Tool, options: &Options) -> Flex {
    let width = options.width(tool).unwrap_or_default();
    let brushing = options.brushing(tool).unwrap_or(Brushing {
        hardness: 1.0,
        opacity: 1.0,
        flow: 1.0,
    });
    row! {
        lead;
        sized(width);
        share(tr!("paint.options.hardness"), brushing.hardness, Setting::Hardness);
        share(tr!("paint.options.opacity"), brushing.opacity, Setting::Opacity);
        if tool == Tool::Paintbrush {
            share(tr!("paint.options.flow"), brushing.flow, Setting::Flow);
        }
        antialiasing(options.antialiased, Paint::set_antialiased);
    }
}

fn freeform(lead: impl View, options: &Options) -> Flex {
    let width = options.width(Tool::FreeformShape).unwrap_or_default();
    row! {
        lead;
        brush(width, options.filling.outlines());
        filled(options.filling);
        antialiasing(options.antialiased, Paint::set_antialiased);
    }
}

fn gradienting(lead: impl View, gradienting: Gradienting) -> Flex {
    row! {
        lead;
        labelled(
            tr!("paint.options.gradient_shape"),
            choice(
                GradientShape::ALL,
                gradienting.shape,
                wording::gradient_shape,
                Paint::set_gradient_shape,
            )
            .width(GRADIENT_SHAPES),
        );
        labelled(
            tr!("paint.options.gradient_mode"),
            choice(
                GradientMode::ALL,
                gradienting.mode,
                wording::gradient_mode,
                Paint::set_gradient_mode,
            )
            .width(GRADIENT_MODES),
        );
    }
}

fn sized(width: f32) -> impl View {
    labelled(
        tr!("paint.options.size"),
        row! {
            slider(width.ln())
                .range(THINNEST.ln()..=THICKEST.ln())
                .control_size(ControlSize::Small)
                .on_change(|paint: &mut Paint, value: f32| paint.slide_width(value.exp().round()))
                .on_commit(|paint: &mut Paint, value: f32| paint.set_width(value.exp().round()))
                .width(SLIDER);
            number_field(width)
                .range(THINNEST..=THICKEST)
                .unit(tr!("paint.unit.pixels"))
                .control_size(ControlSize::Small)
                .on_commit(Paint::set_width)
                .width(FIELD);
        }
        .gap(Space::SNUG)
        .align_items(Align::Center),
    )
}

fn share(label: Label, value: f32, setting: Setting) -> impl View {
    labelled(
        label,
        number_field((value * 100.0).round())
            .range(0.0..=100.0)
            .unit("%")
            .control_size(ControlSize::Small)
            .on_commit(move |paint: &mut Paint, percent: f32| {
                paint.set_brushing(setting, percent / 100.0);
            })
            .width(SHARE),
    )
}

fn antialiasing(on: bool, set: fn(&mut Paint, bool)) -> impl View {
    checkbox(on)
        .label(text(tr!("paint.options.antialiasing")).caption().no_wrap())
        .control_size(ControlSize::Small)
        .on_toggle(set)
}

fn brush(width: f32, enabled: bool) -> impl View {
    labelled(
        tr!("paint.options.width"),
        row! {
            slider(width.ln())
                .range(THINNEST.ln()..=THICKEST.ln())
                .control_size(ControlSize::Small)
                .enabled(enabled)
                .on_change(|paint: &mut Paint, value: f32| paint.slide_width(value.exp().round()))
                .on_commit(|paint: &mut Paint, value: f32| paint.set_width(value.exp().round()))
                .width(SLIDER);
            number_field(width)
                .range(THINNEST..=THICKEST)
                .unit(tr!("paint.unit.pixels"))
                .control_size(ControlSize::Small)
                .enabled(enabled)
                .on_commit(Paint::set_width)
                .width(FIELD);
        }
        .gap(Space::SNUG)
        .align_items(Align::Center),
    )
}

fn choice<T: Copy + PartialEq + 'static, const N: usize>(
    all: [T; N],
    selected: T,
    name: fn(T) -> Label,
    set: fn(&mut Paint, T),
) -> Combo {
    let at = all.iter().position(|one| *one == selected);
    combo(at)
        .options(all.map(name))
        .control_size(ControlSize::Small)
        .focusless()
        .on_select(move |paint: &mut Paint, at: usize| set(paint, all[at]))
}

fn filled(selected: Filling) -> impl View {
    labelled(
        tr!("paint.options.filling"),
        choice(Filling::ALL, selected, wording::filling, Paint::set_filling).width(FILLINGS),
    )
}

fn selecting(selected: Combine) -> impl View {
    labelled(
        tr!("paint.options.selection_mode"),
        choice(SELECTING, selected, wording::combine, Paint::set_selecting).width(SELECTION_MODES),
    )
}

fn zooming(camera: Camera) -> impl View {
    row! {
        button(icon(MaterialSymbol::ZOOM_OUT))
            .ghost()
            .small()
            .icon_only()
            .focusless()
            .on_press(|paint: &mut Paint| paint.zoom(false))
            .tooltip(Command::ZoomOut.tooltip(tr!("paint.command.zoom_out")));
        live_observed(move || {
            let scale = camera.scale();
            let at = PRESETS
                .iter()
                .position(|preset| (preset - scale).abs() < 1e-4);
            combo(at)
                .options(PRESETS.map(wording::percent))
                .placeholder(wording::percent(scale))
                .control_size(ControlSize::Small)
                .focusless()
                .on_select(|paint: &mut Paint, at: usize| paint.zoom_to(PRESETS[at]))
        })
        .width(ZOOMS);
        button(icon(MaterialSymbol::ZOOM_IN))
            .ghost()
            .small()
            .icon_only()
            .focusless()
            .on_press(|paint: &mut Paint| paint.zoom(true))
            .tooltip(Command::ZoomIn.tooltip(tr!("paint.command.zoom_in")));
        button(icon(MaterialSymbol::FIT_SCREEN))
            .ghost()
            .small()
            .icon_only()
            .focusless()
            .on_press(Paint::fit)
            .tooltip(Command::Fit.tooltip(tr!("paint.command.fit")));
    }
    .gap(Space::TIGHT)
    .align_items(Align::Center)
}

fn sampling(lead: impl View, sampling: Sampling, last: Option<Color>) -> Flex {
    row! {
        lead;
        labelled(
            tr!("paint.options.sampling"),
            choice(
                SampleSize::ALL,
                sampling.size,
                wording::sample_size,
                Paint::set_sample_size,
            )
            .width(SAMPLE_SIZES),
        );
        labelled(
            tr!("paint.options.sample_from"),
            choice(
                SampleFrom::ALL,
                sampling.from,
                wording::sample_from,
                Paint::set_sample_from,
            )
            .width(SOURCES),
        );
        if let Some(color) = last {
            labelled(tr!("paint.options.sampled"), row! {
                container(spacer())
                    .background(color)
                    .border(1.0, palette().outline)
                    .radius(Corner::Detail)
                    .size(CHIP, CHIP);
                text(wording::color(color)).caption().no_wrap();
            }
            .gap(Space::TIGHT)
            .align_items(Align::Center));
        }
    }
}

fn flooding(lead: impl View, flooding: Flooding, combine: Option<Combine>) -> Flex {
    row! {
        lead;
        if let Some(combine) = combine {
            selecting(combine);
        }
        labelled(
            tr!("paint.options.tolerance"),
            row! {
                slider(flooding.tolerance * 100.0)
                    .range(0.0..=100.0)
                    .step(1.0)
                    .control_size(ControlSize::Small)
                    .on_change(|paint: &mut Paint, value: f32| paint.set_tolerance(value / 100.0))
                    .width(SLIDER);
                number_field((flooding.tolerance * 100.0).round())
                    .range(0.0..=100.0)
                    .unit("%")
                    .control_size(ControlSize::Small)
                    .on_commit(|paint: &mut Paint, value: f32| paint.set_tolerance(value / 100.0))
                    .width(SHARE);
            }
            .gap(Space::SNUG)
            .align_items(Align::Center),
        );
        checkbox(flooding.mode == FloodMode::Contiguous)
            .label(text(tr!("paint.options.contiguous")).caption().no_wrap())
            .control_size(ControlSize::Small)
            .on_toggle(|paint: &mut Paint, contiguous: bool| {
                paint.set_flood_mode(match contiguous {
                    true => FloodMode::Contiguous,
                    false => FloodMode::Global,
                });
            });
        labelled(
            tr!("paint.options.sample_from"),
            choice(
                SampleFrom::ALL,
                flooding.from,
                wording::sample_from,
                Paint::set_flood_from,
            )
            .width(SOURCES),
        );
        antialiasing(flooding.anti_aliased, Paint::set_flood_anti_aliased);
    }
}

fn lettering(lead: impl View, letters: &Letters) -> Flex {
    row! {
        lead;
        font_choice(&letters.font, Paint::set_font).width(FONT);
        labelled(
            tr!("paint.options.size"),
            number_field(letters.size)
                .range(Lettering::SMALLEST..=Lettering::LARGEST)
                .unit(tr!("paint.unit.pixels"))
                .control_size(ControlSize::Small)
                .on_commit(Paint::set_text_size)
                .width(FIELD),
        );
        row! {
            styled(
                MaterialSymbol::FORMAT_BOLD,
                tr!("paint.options.bold"),
                letters.bold,
                Paint::toggle_bold,
            );
            styled(
                MaterialSymbol::FORMAT_ITALIC,
                tr!("paint.options.italic"),
                letters.italic,
                Paint::toggle_italic,
            );
        }
        .gap(Space::TIGHT)
        .align_items(Align::Center);
        align_choice(letters.align, Paint::set_text_align).width(ALIGNMENTS);
        antialiasing(letters.antialias, Paint::set_antialias);
        row! {
            checkbox(letters.wraps)
                .label(text(tr!("paint.options.wrap")).caption().no_wrap())
                .control_size(ControlSize::Small)
                .on_toggle(Paint::set_wraps);
            number_field(letters.box_width as f32)
                .range(1.0..=BOX_WIDEST)
                .unit(tr!("paint.unit.pixels"))
                .control_size(ControlSize::Small)
                .enabled(letters.wraps)
                .on_commit(Paint::set_box_width)
                .width(FIELD);
        }
        .gap(Space::SNUG)
        .align_items(Align::Center);
    }
}

fn styled(symbol: MaterialSymbol, name: Label, on: bool, toggle: fn(&mut Paint)) -> impl View {
    let accent = palette().accent;
    let (fill, ink) = match on {
        true => (accent.scale_alpha(ACTIVE_TINT), accent),
        false => (Color::TRANSPARENT, palette().text),
    };
    container(icon(symbol).extent(Length::px(STYLE_GLYPH)).color(ink))
        .background(fill)
        .radius(Corner::Item)
        .center()
        .on_press(toggle)
        .focusless()
        .size(STYLE_TILE, STYLE_TILE)
        .tooltip(name)
}
