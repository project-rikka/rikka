use rikka_app::prelude::*;

use crate::shell::Shell;

const EXCEPTIONS: &str = "rikka-browser-http-allowed";

pub(crate) fn view(shell: &Shell) -> Element {
    let browser = shell.browser();
    let on = browser.preferences().https_only();
    let profile = shell.persistent_profile();
    let exceptions = match profile {
        Some(profile) => {
            let store = browser.store();
            let revision = shell.sections.exceptions();
            match fetch_blocking((EXCEPTIONS, profile, revision), move || {
                store.flush()?;
                store.https_only_exceptions(profile)
            }) {
                Fetch::Ready(hosts) => hosts.to_vec(),
                _ => Vec::new(),
            }
        }
        None => Vec::new(),
    };
    let rows: Vec<Element> = exceptions
        .into_iter()
        .map(|host| {
            let forbidden = host.clone();
            field(
                host.clone(),
                button(text(tr!("browser.https.remove")))
                    .ghost()
                    .small()
                    .on_press(move |shell: &mut Shell| {
                        if let Some(profile) = shell.persistent_profile() {
                            shell.browser().forbid_http(profile, &forbidden);
                            shell.sections.exceptions_changed();
                        }
                    }),
            )
            .build()
            .keyed(host)
        })
        .collect();
    let listed = match rows.is_empty() {
        true => text(tr!("browser.https.no-exceptions")).muted().any(),
        false => column_of(rows).gap(Space::TIGHT).fill_width().any(),
    };
    page([
        section(
            tr!("browser.https.title"),
            field(
                tr!("browser.https.upgrade"),
                switch(on)
                    .on_toggle(|shell: &mut Shell, on: bool| shell.browser().set_https_only(on)),
            ),
        ),
        section(tr!("browser.https.exceptions"), listed),
    ])
    .any()
}
