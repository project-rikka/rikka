use rikka_app::prelude::*;
use rikka_session::remote::{Address, Encoding, Reach, RemoteStatus, Viewer};
use rikka_widgets::{column, row};

use crate::info_row::info_row;
use crate::remote::component::Remote;

const GLYPH: f32 = 20.0;
const ACTION_GLYPH: f32 = 14.0;
const MEGABIT: f64 = 1_000_000.0;
const BREAK_AFTER_COLON: &str = ":\u{200B}";

pub(crate) fn state(remote: &Remote) -> Option<impl View> {
    let palette = palette();
    let problem = remote.status().and_then(|status| status.problem.as_deref());
    let failure = remote.failure();
    if problem.is_none() && failure.is_none() {
        return None;
    }
    Some(
        column! {
            problem.map(|problem| {
                banner(tr!("settings.remote.problem", reason = problem))
                    .symbol(MaterialSymbol::WARNING)
                    .tint(palette.warning)
            });
            failure.map(|reason| {
                banner(tr!("settings.remote.failed", reason = reason)).tint(palette.danger)
            });
        }
        .gap(Space::SNUG),
    )
}

pub(crate) fn addresses(remote: &Remote) -> Option<impl View> {
    let serving = remote.status()?.serving.as_ref()?;
    Some(section(
        tr!("settings.remote.addresses"),
        column! {
            for served in &serving.addresses {
                address(served);
            }
            labelled(
                MaterialSymbol::KEY,
                tr!("settings.remote.fingerprint"),
                &serving.fingerprint,
            );
        }
        .gap(Space::NORMAL)
        .fill_width(),
    ))
}

fn address(served: &Address) -> impl View {
    let (symbol, label) = match served.reach {
        Reach::Local => (MaterialSymbol::LAPTOP, tr!("settings.remote.address.local")),
        Reach::Tailnet => (
            MaterialSymbol::VPN_LOCK,
            tr!("settings.remote.address.tailnet"),
        ),
        Reach::Network => (MaterialSymbol::LAN, tr!("settings.remote.address.network")),
    };
    labelled(symbol, label, &served.address)
}

fn labelled(symbol: MaterialSymbol, label: Label, value: &str) -> impl View {
    row! {
        icon(symbol)
            .extent(Length::px(GLYPH))
            .color(palette().text_muted);
        column! {
            text(label).caption().muted();
            text(breakable(value))
                .label()
                .family(Family::parse(font::JETBRAINS_MONO));
        }
        .gap(Space::HAIR)
        .grow(1.0);
        copy(value);
    }
    .gap(Space::NORMAL)
    .align_items(Align::Center)
    .fill_width()
}

fn breakable(value: &str) -> String {
    value.replace(':', BREAK_AFTER_COLON)
}

pub(crate) fn viewer(remote: &Remote) -> Option<impl View> {
    let status: &RemoteStatus = remote.status()?;
    Some(section(
        tr!("settings.remote.viewer"),
        match &status.viewer {
            Some(viewer) => watching(viewer).build(),
            None => text(tr!("settings.remote.nobody")).label().muted().build(),
        },
    ))
}

fn watching(viewer: &Viewer) -> impl View {
    let since = rikka_format::format_timestamp(viewer.since);
    column! {
        row! {
            icon(MaterialSymbol::PERSON)
                .extent(Length::px(GLYPH))
                .color(palette().accent);
            column! {
                text(viewer.device.clone()).label().medium();
                text(tr!(
                    "settings.remote.since",
                    address = viewer.address.as_str(),
                    time = since.as_str()
                ))
                .caption()
                .muted();
            }
            .gap(Space::HAIR)
            .grow(1.0);
            button(text(tr!("settings.remote.disconnect")))
                .secondary()
                .small()
                .on_press(|remote: &mut Remote| remote.disconnect());
        }
        .gap(Space::NORMAL)
        .align_items(Align::Center)
        .fill_width();
        viewer.encoding.as_ref().and_then(|encoding| {
            info_row(tr!("settings.remote.picture"), picture(encoding))
        });
    }
    .gap(Space::NORMAL)
    .fill_width()
}

fn picture(encoding: &Encoding) -> String {
    tr!(
        "settings.remote.picture-reading",
        codec = encoding.codec.to_string().as_str(),
        width = encoding.width,
        height = encoding.height,
        megabits = format!("{:.1}", encoding.bitrate as f64 / MEGABIT).as_str()
    )
    .to_string()
}

fn copy(what: &str) -> Button {
    let what = what.to_owned();
    button(
        row! {
            icon(MaterialSymbol::CONTENT_COPY).extent(Length::px(ACTION_GLYPH));
            text(tr!("settings.remote.copy")).caption();
        }
        .gap(Space::TIGHT)
        .align_items(Align::Center),
    )
    .ghost()
    .small()
    .on_press(move || set_text_of(Selection::Clipboard, &what))
}
