use cut_project::{Art, ArtField, Clip, Graphic, Property};
use rikka_app::prelude::*;
use rikka_effects::{Scale, Unit};
use rikka_figure::{Band, Lettering, Pen, Shadow, TextAlign};
use rikka_figure_editing::{align_choice, font_choice};
use rikka_widgets::{column, row};

use super::keys::Keying;
use super::rows::{
    FIELD, colour, colour_with, group, number_with, sliding, sliding_with, switch_row,
};
use crate::cut::Cut;
use crate::{art, wording};

type Diamonds<'a> = &'a dyn Fn(ArtField) -> Option<Element>;

const TEXT_HEIGHTS: std::ops::RangeInclusive<f32> = 64.0..=220.0;
const WIDEST_EDGE: f32 = 200.0;
const FARTHEST: f32 = 500.0;
const BLURRIEST: f32 = 200.0;
const ROUNDEST: f32 = 500.0;
const FONT: f32 = 180.0;
fn lettered(graphic: &mut Graphic, change: impl FnOnce(&mut Lettering)) {
    if let Art::Text { lettering, .. } = &mut graphic.art {
        change(lettering);
    }
}

fn edge(lettering: &Lettering, at: usize) -> Pen {
    lettering.outlines.get(at).copied().unwrap_or(Pen::new(
        match at {
            0 => Color::BLACK,
            _ => Color::WHITE,
        },
        art::edge_width(lettering.size),
    ))
}

fn edged(graphic: &mut Graphic, at: usize, change: impl FnOnce(&mut Pen)) {
    lettered(graphic, |lettering| {
        if let Some(pen) = lettering.outlines.get_mut(at) {
            change(pen);
        }
    });
}

fn shaded(graphic: &mut Graphic, change: impl FnOnce(&mut Shadow)) {
    lettered(graphic, |lettering| {
        if let Some(shadow) = &mut lettering.shadow {
            change(shadow);
        }
    });
}

fn banded(graphic: &mut Graphic, change: impl FnOnce(&mut Band)) {
    lettered(graphic, |lettering| {
        if let Some(band) = &mut lettering.band {
            change(band);
        }
    });
}

pub(crate) fn text(
    cut: &Cut,
    (lettering, reveal): (&Lettering, f32),
    clip: &Clip,
    keying: Keying,
) -> impl View {
    let diamond = |field: ArtField| Some(keying.on(clip, Property::Art(field)));
    column_of([
        writing(cut, lettering, &diamond).build(),
        letters(lettering, reveal, &diamond).build(),
        edges(lettering).build(),
        shadow(lettering).build(),
        band(lettering).build(),
    ])
    .gap(Space::ROOMY)
    .fill_width()
}

fn writing(cut: &Cut, lettering: &Lettering, diamond: Diamonds<'_>) -> impl View {
    let editor = cut.draft().map(|draft| {
        text_editor(&draft.document)
            .gutter(false)
            .wrap(true)
            .fits_text(TEXT_HEIGHTS)
            .font_size(14.0)
            .on_blur(Cut::commit_text)
            .fill_width()
    });
    let pixels = Unit::Pixels;
    group(
        tr!("cut.panel.text"),
        vec![
            column! {
                if let Some(editor) = editor {
                    editor;
                }
            }
            .fill_width()
            .build(),
            field(
                tr!("cut.panel.font"),
                font_choice(&lettering.font, |cut: &mut Cut, font: String| {
                    cut.restyle(|graphic: &mut Graphic| {
                        lettered(graphic, |lettering| lettering.font = font);
                    });
                })
                .width(FONT),
            )
            .build(),
            number_with(
                wording::art_field(ArtField::Size, false),
                lettering.size,
                Scale::new(Lettering::SMALLEST..=Lettering::LARGEST).unit(pixels),
                |graphic: &mut Graphic, size: f32| lettered(graphic, |lettering| lettering.size = size),
                diamond(ArtField::Size),
            ),
            row! {
                switch_row(
                    tr!("cut.panel.bold"),
                    lettering.weight >= Lettering::BOLD,
                    |graphic: &mut Graphic, on: bool| {
                        lettered(graphic, |lettering| {
                            lettering.weight = match on {
                                true => Lettering::BOLD,
                                false => Lettering::REGULAR,
                            };
                        });
                    },
                );
                switch_row(
                    tr!("cut.panel.italic"),
                    lettering.italic,
                    |graphic: &mut Graphic, on: bool| lettered(graphic, |lettering| lettering.italic = on),
                );
            }
            .gap(Space::ROOMY)
            .build(),
            field(
                tr!("cut.panel.align"),
                align_choice(lettering.align, |cut: &mut Cut, align: TextAlign| {
                    cut.restyle(|graphic: &mut Graphic| {
                        lettered(graphic, |lettering| lettering.align = align);
                    });
                })
                .width(FIELD),
            )
            .build(),
        ],
    )
}
fn letters(lettering: &Lettering, reveal: f32, diamond: Diamonds<'_>) -> impl View {
    let pixels = Unit::Pixels;
    group(
        tr!("cut.panel.letters"),
        vec![
            colour_with(
                wording::art_field(ArtField::Color, false),
                lettering.color,
                |graphic: &mut Graphic, color: Color| {
                    lettered(graphic, |lettering| lettering.color = color)
                },
                diamond(ArtField::Color),
            ),
            sliding_with(
                wording::art_field(ArtField::Spacing, false),
                lettering.letter_spacing,
                Scale::new(-(lettering.size / 2.0).floor()..=lettering.size * 2.0)
                    .unit(pixels)
                    .decimals(1),
                |graphic: &mut Graphic, spacing: f32| {
                    lettered(graphic, |lettering| lettering.letter_spacing = spacing);
                },
                diamond(ArtField::Spacing),
            ),
            sliding(
                tr!("cut.panel.line_spacing"),
                lettering.line_spacing * 100.0,
                Scale::new(Lettering::TIGHTEST_LINES * 100.0..=Lettering::LOOSEST_LINES * 100.0)
                    .unit(Unit::Percent),
                |graphic: &mut Graphic, spacing: f32| {
                    lettered(graphic, |lettering| {
                        lettering.line_spacing = spacing / 100.0
                    });
                },
            ),
            sliding_with(
                wording::art_field(ArtField::Reveal, false),
                reveal * 100.0,
                Scale::new(0.0..=100.0).unit(Unit::Percent).decimals(1),
                |graphic: &mut Graphic, percent: f32| {
                    if let Art::Text { reveal, .. } = &mut graphic.art {
                        *reveal = percent / 100.0;
                    }
                },
                diamond(ArtField::Reveal),
            ),
        ],
    )
}

fn edges(lettering: &Lettering) -> impl View {
    let mut rows = Vec::new();
    for at in 0..Lettering::MAX_OUTLINES {
        let outline = lettering.outlines.get(at).copied();
        let label = match at {
            0 => tr!("cut.panel.edge"),
            _ => tr!("cut.panel.second_edge"),
        };
        if at > lettering.outlines.len() {
            break;
        }
        let fresh = edge(lettering, at);
        rows.push(switch_row(
            label,
            outline.is_some(),
            move |graphic: &mut Graphic, on: bool| {
                lettered(graphic, |lettering| match on {
                    true if lettering.outlines.len() == at => lettering.outlines.push(fresh),
                    true => {}
                    false => lettering.outlines.truncate(at),
                });
            },
        ));
        let Some(pen) = outline else {
            continue;
        };
        rows.push(colour(
            tr!("cut.panel.color"),
            pen.color,
            move |graphic: &mut Graphic, color: Color| edged(graphic, at, |pen| pen.color = color),
        ));
        rows.push(sliding(
            tr!("cut.panel.edge_width"),
            pen.width,
            Scale::new(1.0..=WIDEST_EDGE).unit(Unit::Pixels),
            move |graphic: &mut Graphic, width: f32| edged(graphic, at, |pen| pen.width = width),
        ));
    }
    group(tr!("cut.panel.edges"), rows)
}

fn shadow(lettering: &Lettering) -> impl View {
    let fresh = Shadow {
        color: Color::rgba(0.0, 0.0, 0.0, 0.6),
        offset: Point::new(lettering.size * 0.06, lettering.size * 0.06),
        blur: lettering.size * 0.08,
    };
    let mut rows = vec![switch_row(
        tr!("cut.panel.shadow"),
        lettering.shadow.is_some(),
        move |graphic: &mut Graphic, on: bool| {
            lettered(graphic, |lettering| lettering.shadow = on.then_some(fresh));
        },
    )];
    if let Some(shadow) = lettering.shadow {
        let pixels = Unit::Pixels;
        rows.extend([
            colour(
                tr!("cut.panel.color"),
                shadow.color,
                |graphic: &mut Graphic, color: Color| {
                    shaded(graphic, |shadow| shadow.color = color)
                },
            ),
            sliding(
                tr!("cut.panel.blur"),
                shadow.blur,
                Scale::new(0.0..=BLURRIEST).unit(pixels).decimals(1),
                |graphic: &mut Graphic, blur: f32| shaded(graphic, |shadow| shadow.blur = blur),
            ),
            sliding(
                tr!("cut.panel.offset_x"),
                shadow.offset.x,
                Scale::new(-FARTHEST..=FARTHEST).unit(pixels).decimals(1),
                |graphic: &mut Graphic, x: f32| shaded(graphic, |shadow| shadow.offset.x = x),
            ),
            sliding(
                tr!("cut.panel.offset_y"),
                shadow.offset.y,
                Scale::new(-FARTHEST..=FARTHEST).unit(pixels).decimals(1),
                |graphic: &mut Graphic, y: f32| shaded(graphic, |shadow| shadow.offset.y = y),
            ),
        ]);
    }
    group(tr!("cut.panel.shadow_group"), rows)
}

fn band(lettering: &Lettering) -> impl View {
    let fresh = Band {
        color: Color::rgba(0.0, 0.0, 0.0, 0.6),
        padding: (lettering.size * 0.25).round(),
        radius: (lettering.size * 0.15).round(),
    };
    let mut rows = vec![switch_row(
        tr!("cut.panel.band"),
        lettering.band.is_some(),
        move |graphic: &mut Graphic, on: bool| {
            lettered(graphic, |lettering| lettering.band = on.then_some(fresh));
        },
    )];
    if let Some(band) = lettering.band {
        let pixels = Unit::Pixels;
        rows.extend([
            colour(
                tr!("cut.panel.color"),
                band.color,
                |graphic: &mut Graphic, color: Color| banded(graphic, |band| band.color = color),
            ),
            sliding(
                tr!("cut.panel.padding"),
                band.padding,
                Scale::new(0.0..=FARTHEST).unit(pixels),
                |graphic: &mut Graphic, padding: f32| {
                    banded(graphic, |band| band.padding = padding)
                },
            ),
            sliding(
                tr!("cut.panel.corner"),
                band.radius,
                Scale::new(0.0..=ROUNDEST).unit(pixels),
                |graphic: &mut Graphic, radius: f32| banded(graphic, |band| band.radius = radius),
            ),
        ]);
    }
    group(tr!("cut.panel.band_group"), rows)
}
