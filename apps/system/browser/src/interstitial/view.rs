use rikka_app::prelude::*;
use rikka_widgets::{column, row};

use crate::interstitial::{Ended, Problem, network};
use crate::shell::Shell;
use browser_model::TabId;
use browser_pages::ErrorAction;

const WIDEST: f32 = 560.0;
const GLYPH: f32 = 48.0;

pub(crate) fn title(problem: &Problem) -> Label {
    match problem {
        Problem::Certificate { .. } => tr!("browser.interstitial.certificate-title"),
        Problem::HttpsOnly { .. } => tr!("browser.interstitial.https-title"),
        Problem::Network { kind, .. } => network::title(*kind),
        Problem::Gone(Ended::Crashed | Ended::Killed) => tr!("browser.interstitial.crashed-title"),
        Problem::Gone(Ended::EngineExited) => tr!("browser.interstitial.ended-title"),
    }
}

pub(crate) fn view(problem: &Problem, tab: TabId) -> Container {
    let body = match problem {
        Problem::Certificate { host } => framed(
            MaterialSymbol::WARNING,
            palette().danger,
            title(problem),
            host.clone(),
            column! {
                text(tr!("browser.interstitial.certificate-message")).caption().muted();
                row! {
                    button(text(tr!("browser.interstitial.back")))
                        .primary()
                        .on_press(move |shell: &mut Shell| shell.back_to_safety(tab));
                };
            }
            .gap(Space::NORMAL)
            .any(),
        ),
        Problem::HttpsOnly { host, http } => {
            let (continued_host, http) = (host.clone(), http.clone());
            framed(
                MaterialSymbol::LOCK_OPEN,
                palette().warning,
                title(problem),
                host.clone(),
                row! {
                    button(text(tr!("browser.interstitial.back")))
                        .primary()
                        .on_press(move |shell: &mut Shell| shell.back_to_safety(tab));
                    button(text(tr!("browser.interstitial.continue-http")))
                        .secondary()
                        .on_press(move |shell: &mut Shell| {
                            shell.continue_over_http(tab, continued_host.clone(), http.clone())
                        });
                }
                .gap(Space::SNUG)
                .any(),
            )
        }
        Problem::Network { uri, host, kind } => {
            let reading = network::presentation(*kind);
            framed(
                reading.symbol,
                palette().text_muted,
                title(problem),
                host.clone(),
                column! {
                    network::message(*kind).map(|message| text(message).caption().muted());
                    action(reading.action, tab, uri);
                }
                .gap(Space::NORMAL)
                .any(),
            )
        }
        Problem::Gone(_) => framed(
            MaterialSymbol::ERROR,
            palette().text_muted,
            title(problem),
            String::new(),
            row! {
                button(text(tr!("browser.interstitial.reload")))
                    .primary()
                    .on_press(move |shell: &mut Shell| shell.reload_gone(tab));
            }
            .any(),
        ),
    };
    container(body)
        .center()
        .padding(Inset::all(Space::WIDE))
        .background(palette().background)
        .on_press(skip_view)
        .focusless()
}

fn action(action: ErrorAction, tab: TabId, uri: &str) -> Element {
    let uri = uri.to_owned();
    let action = match action {
        ErrorAction::Retry => button(text(tr!("browser.interstitial.retry")))
            .on_press(move |shell: &mut Shell| shell.retry(tab, uri.clone())),
        ErrorAction::Back => button(text(tr!("browser.interstitial.go-back")))
            .on_press(move |shell: &mut Shell| shell.back_to_safety(tab)),
        ErrorAction::OpenOutside => button(text(tr!("browser.interstitial.open-outside")))
            .on_press(move |shell: &mut Shell| shell.open_outside(tab, uri.clone())),
    };
    row_of([action.primary().any()]).any()
}

fn framed(
    symbol: MaterialSymbol,
    tint: Color,
    title: Label,
    host: String,
    actions: Element,
) -> Element {
    column! {
        icon(symbol).extent(Length::px(GLYPH)).color(tint);
        text(title).title();
        (!host.is_empty()).then(|| text(host).muted());
        actions;
    }
    .gap(Space::NORMAL)
    .max_width(WIDEST)
    .fill_width()
    .any()
}
