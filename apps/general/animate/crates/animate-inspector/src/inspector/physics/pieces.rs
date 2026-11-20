use animate_cast::{Figure, Garment};
use animate_ui::framed;
use rikka_app::prelude::*;
use rikka_physics_tailor::{AnalyzedPiece, PieceKind, Preset};

use crate::{Desk, Reach, wording};

const LAYERS: u8 = 4;
const LAYER_WIDTH: f32 = 100.0;
const GLYPH: f32 = 16.0;

pub(super) fn pieces(desk: Desk<'_>, figure: &Figure) -> Element {
    let mut parts = vec![text(tr!("animate.cloth.pieces")).bold().no_wrap().any()];
    let cloth = &figure.cloth;
    match (cloth.analysis(), cloth.error()) {
        (_, Some(error)) => {
            parts.push(text(animate_ui::error_chain(error)).caption().muted().any())
        }
        (Some(analysis), None) if analysis.pieces().is_empty() => {
            parts.push(text(tr!("animate.cloth.bare")).caption().muted().any())
        }
        (Some(analysis), None) => parts.extend(analysis.pieces().iter().map(|piece| {
            let garment = Garment::new(cloth.cloth_settings(), piece);
            row(
                desk.reach(),
                piece,
                &garment,
                desk.controls().piece == Some(piece.key),
            )
        })),
        (None, None) => {}
    }
    framed(parts)
}

fn row(reach: Reach, piece: &AnalyzedPiece, garment: &Garment, chosen: bool) -> Element {
    let key = piece.key;
    let kind = match piece.kind {
        PieceKind::Sheet => MaterialSymbol::CHECKROOM,
        PieceKind::Strands => MaterialSymbol::FACE_3,
    };
    let name = button(
        row_of([
            icon(kind).extent(Length::px(GLYPH)).any(),
            text(piece.name.clone()).no_wrap().any(),
        ])
        .gap(Space::TIGHT)
        .align_items(Align::Center),
    )
    .small()
    .focusless()
    .on_press(reach.act(move |studio| studio.choose_piece(key)));
    let name = match chosen {
        true => name.primary(),
        false => name.ghost(),
    };
    let preset = Preset::ALL
        .iter()
        .position(|preset| *preset == garment.preset);
    let layers = std::iter::once(tr!("animate.cloth.layer_auto"))
        .chain((1..=LAYERS).map(|layer| tr!("animate.cloth.layer", number = u32::from(layer))));
    let layer = garment.layer.map_or(0, |layer| usize::from(layer) + 1);
    column_of([
        row_of([
            container(name).grow(1.0).any(),
            switch(garment.enabled)
                .control_size(ControlSize::Small)
                .on_toggle(reach.act_with(move |studio, on: bool| {
                    studio.set_piece_enabled(key, on);
                }))
                .any(),
        ])
        .gap(Space::TIGHT)
        .align_items(Align::Center)
        .fill_width()
        .any(),
        row_of([
            container(
                combo(preset)
                    .options(Preset::ALL.map(wording::garment))
                    .on_select(reach.act_with(move |studio, index: usize| {
                        if let Some(preset) = Preset::ALL.get(index) {
                            studio.set_preset(key, *preset);
                        }
                    }))
                    .focusless()
                    .control_size(ControlSize::Small)
                    .fill_width(),
            )
            .grow(1.0)
            .any(),
            combo(Some(layer))
                .options(layers)
                .on_select(reach.act_with(move |studio, index: usize| {
                    let layer = index
                        .checked_sub(1)
                        .and_then(|layer| u8::try_from(layer).ok());
                    studio.set_layer(key, layer);
                }))
                .focusless()
                .control_size(ControlSize::Small)
                .width(LAYER_WIDTH)
                .any(),
        ])
        .gap(Space::TIGHT)
        .fill_width()
        .any(),
    ])
    .gap(Space::NONE)
    .fill_width()
    .any()
}
