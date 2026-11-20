use rikka_mcp::{Server, ServerConfigError};

use rikka_widgets::{column, row};

use crate::prelude::*;
use crate::servers::draft::{Draft, Fault, Kind};
use crate::servers::offers::offers;
use crate::servers::state::Servers;
use crate::servers::trial::Trial;

const GLYPH: f32 = 18.0;
const KEY: f32 = 0.4;
const PORT: f32 = 96.0;

pub(crate) fn form(draft: &Draft, checked: &Result<Server, Fault>) -> impl View {
    container(
        column! {
            labelled(
                tr!("chat.servers.name"),
                entry(draft.name(), tr!("chat.servers.name-hint")).autofocus(true),
            );
            segmented(draft.kind().at())
                .segments(Kind::ALL.map(Kind::title))
                .on_select(|servers: &mut Servers, at: usize| {
                    servers.choose(Kind::ALL.get(at).copied().unwrap_or_default());
                });
            if draft.kind() == Kind::Command {
                labelled(
                    tr!("chat.servers.line"),
                    entry(draft.line(), tr!("chat.servers.line-hint")),
                );
                labelled(
                    tr!("chat.servers.folder"),
                    entry(draft.folder(), tr!("chat.servers.folder-hint"))
                        .trailing(glyph(MaterialSymbol::FOLDER_OPEN).on_press(Servers::browse)),
                );
                pairs(
                    draft,
                    tr!("chat.servers.variables"),
                    tr!("chat.servers.variable-hint"),
                    tr!("chat.servers.variable-add"),
                );
            } else {
                labelled(
                    tr!("chat.servers.url"),
                    entry(draft.url(), tr!("chat.servers.url-hint")),
                );
                pairs(
                    draft,
                    tr!("chat.servers.headers"),
                    tr!("chat.servers.header-hint"),
                    tr!("chat.servers.header-add"),
                );
                advanced(draft);
            }
            text(tr!("chat.servers.expanding")).caption().muted();
            if let Some(message) = faulted(checked) {
                text(message).caption().danger();
            }
            trial(draft.trial());
            foot(draft, checked);
        }
        .gap(Space::NORMAL)
        .fill_width(),
    )
    .surface()
    .radius(Corner::Panel)
    .border(1.0, palette().outline)
    .padding(Space::NORMAL)
    .fill_width()
}

fn labelled(label: Label, control: impl View) -> Flex {
    column! {
        text(label).caption().muted();
        control;
    }
    .gap(Space::TIGHT)
    .fill_width()
}

fn entry(document: &TextDocument, hint: Label) -> TextField {
    text_field(document)
        .placeholder(hint)
        .control_size(ControlSize::Small)
        .on_change(Servers::edited)
        .on_submit(Servers::keep)
        .fill_width()
}

fn glyph(symbol: MaterialSymbol) -> Button {
    button(
        icon(symbol)
            .extent(Length::px(GLYPH))
            .color(palette().text_muted),
    )
    .icon_only()
    .ghost()
    .small()
}

fn pairs(draft: &Draft, label: Label, hint: Label, add: Label) -> impl View {
    column! {
        text(label).caption().muted();
        for (at, pair) in draft.pairs().iter().enumerate() {
            row! {
                text_field(pair.key())
                    .placeholder(hint.clone())
                    .control_size(ControlSize::Small)
                    .on_change(Servers::edited)
                    .width(Length::portion(KEY));
                text_field(pair.value())
                    .placeholder(tr!("chat.servers.value-hint"))
                    .control_size(ControlSize::Small)
                    .secret(!pair.is_shown())
                    .trailing(
                        eye(pair.is_shown())
                            .on_press(move |servers: &mut Servers| servers.toggle_pair(at)),
                    )
                    .on_change(Servers::edited)
                    .grow(1.0);
                glyph(MaterialSymbol::CLOSE)
                    .on_press(move |servers: &mut Servers| servers.drop_pair(at));
            }
            .gap(Space::SNUG)
            .align_items(Align::Center)
            .fill_width();
        }
        adding(add, Servers::add_pair);
    }
    .gap(Space::TIGHT)
    .fill_width()
}

fn adding<M>(label: Label, press: impl IntoHandler<(), (), M>) -> Button {
    button(
        row! {
            icon(MaterialSymbol::ADD).extent(Length::px(GLYPH));
            text(label).caption();
        }
        .gap(Space::TIGHT)
        .align_items(Align::Center),
    )
    .ghost()
    .small()
    .on_press(press)
}

fn advanced(draft: &Draft) -> impl View {
    let signing = draft.signing();
    column! {
        button(
            row! {
                text(tr!("chat.servers.oauth")).caption().muted();
                icon(match draft.is_advanced() {
                    true => MaterialSymbol::EXPAND_LESS,
                    false => MaterialSymbol::EXPAND_MORE,
                })
                .extent(Length::px(GLYPH))
                .color(palette().text_muted);
            }
            .gap(Space::HAIR)
            .align_items(Align::Center),
        )
        .ghost()
        .small()
        .on_press(Servers::unfold_advanced);
        if draft.is_advanced() {
            text(tr!("chat.servers.oauth-about")).caption().muted();
            labelled(
                tr!("chat.servers.client"),
                entry(signing.client(), tr!("chat.servers.client-hint")),
            );
            row! {
                labelled(
                    tr!("chat.servers.secret"),
                    entry(signing.secret(), tr!("chat.servers.secret-hint"))
                        .secret(!signing.is_shown())
                        .trailing(eye(signing.is_shown()).on_press(Servers::toggle_secret)),
                )
                .grow(1.0);
                labelled(
                    tr!("chat.servers.port"),
                    entry(signing.port(), tr!("chat.servers.port-hint")).fill_width(),
                )
                .width(Length::px(PORT));
            }
            .gap(Space::SNUG)
            .fill_width();
        }
    }
    .gap(Space::TIGHT)
    .fill_width()
}

fn eye(visible: bool) -> Button {
    glyph(match visible {
        true => MaterialSymbol::VISIBILITY_OFF,
        false => MaterialSymbol::VISIBILITY,
    })
}

fn faulted(checked: &Result<Server, Fault>) -> Option<Label> {
    match checked.as_ref().err()? {
        Fault::ServerConfigError(ServerConfigError::NonHttpUrl) => {
            Some(tr!("chat.servers.unwebbed"))
        }
        Fault::ServerConfigError(_) => None,
        Fault::Unclosed => Some(tr!("chat.servers.unclosed")),
        Fault::Keyless => Some(tr!("chat.servers.keyless")),
        Fault::Repeated(key) => Some(tr!("chat.servers.repeated", key = key)),
        Fault::Port => Some(tr!("chat.servers.port-wrong")),
        Fault::Taken => Some(tr!("chat.servers.taken")),
    }
}

fn trial(trial: &Trial) -> Option<Element> {
    let palette = palette();
    let status_row = |label: Label, colour: Color, symbol: MaterialSymbol| {
        row! {
            icon(symbol).extent(Length::px(GLYPH)).color(colour);
            text(label).caption().color(colour);
        }
        .gap(Space::TIGHT)
        .align_items(Align::Center)
    };
    match trial {
        Trial::Idle | Trial::Trying(_) => None,
        Trial::Reached(offered) => {
            #[expect(
                clippy::cast_possible_wrap,
                reason = "a count of tools, which no server offers more of than an i64 holds"
            )]
            let count = offered.len() as i64;
            Some(
                column! {
                    status_row(
                        tr!("chat.servers.reached", count = count),
                        palette.success,
                        MaterialSymbol::CHECK_CIRCLE,
                    );
                    offers(offered);
                }
                .gap(Space::TIGHT)
                .fill_width()
                .any(),
            )
        }
        Trial::Locked => Some(
            status_row(
                tr!("chat.servers.reached-locked"),
                palette.warning,
                MaterialSymbol::LOCK,
            )
            .any(),
        ),
        Trial::Failed(reason) => Some(
            banner(reason.clone())
                .symbol(MaterialSymbol::ERROR)
                .tint(palette.danger)
                .any(),
        ),
    }
}

fn foot(draft: &Draft, checked: &Result<Server, Fault>) -> impl View {
    let testable = matches!(checked, Ok(_) | Err(Fault::Taken));
    row! {
        button(text(tr!("chat.servers.test")).caption())
            .secondary()
            .small()
            .enabled(testable)
            .loading(draft.trial().is_trying())
            .on_press(Servers::try_out);
        spacer().grow(1.0);
        button(text(tr!("chat.servers.cancel")).caption())
            .ghost()
            .small()
            .on_press(Servers::dismiss);
        button(
            text(match draft.was() {
                Some(_) => tr!("chat.servers.save"),
                None => tr!("chat.servers.add"),
            })
            .caption(),
        )
        .small()
        .enabled(checked.is_ok())
        .on_press(Servers::keep);
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
    .fill_width()
}
