mod address;
mod alerts;
mod boot;
mod browser;
mod command;
mod command_bar;
mod content;
mod corners;
mod default_browser;
mod downloads;
mod engine_settings;
mod engines;
mod extensions;
mod favicons;
mod find;
mod geometry;
mod gestures;
mod home;
mod https;
mod internal;
mod interstitial;
mod keys;
mod launch;
mod link;
mod little;
mod memory_saver;
mod menus;
mod model;
mod omnibox;
mod overlays;
mod page;
mod page_services;
mod peek;
mod reader;
mod relay;
mod requests;
mod search;
mod sections;
mod services;
mod shell;
mod sidebar;
mod site_info;
mod split;
mod startup;
mod status;
mod topbar;
mod translate;
mod widgets;

use std::rc::Rc;

use rikka_app::prelude::*;

use crate::boot::Boot;
use crate::launch::{Kind, Launch};
use crate::shell::{Shell, WindowContent};

fn open_windows(boot: &Rc<Boot>, open: LaunchRequest) -> Vec<Window<Shell>> {
    let launch = Launch::from_open(&open);
    if little::wants_little(&launch) {
        return launch
            .addresses
            .into_iter()
            .map(|address| {
                let content = WindowContent::Little(None, little::Little::Handed(address));
                match boot.ready() {
                    Some(browser) => shell::window(browser, false, content),
                    None => shell::first_window(Rc::clone(boot), false, content),
                }
            })
            .collect();
    }
    let Launch {
        kind,
        addresses,
        token,
    } = launch;
    let private = kind == Kind::PrivateWindow;
    let Some(browser) = boot.ready() else {
        return vec![shell::first_window(
            Rc::clone(boot),
            private,
            WindowContent::Start(addresses),
        )];
    };
    let seeded = |addresses: Vec<String>| match addresses.is_empty() {
        true => WindowContent::Blank,
        false => WindowContent::Addresses(addresses),
    };
    if kind == Kind::Main
        && let Some(sender) = browser
            .model()
            .windows
            .main()
            .and_then(|main| browser.sender(main))
    {
        let _ = sender.send(move |shell: &mut Shell| shell.open_addresses(addresses, token));
        return Vec::new();
    }
    if private {
        return vec![shell::window(browser, true, seeded(addresses))];
    }
    match browser
        .take_reopening()
        .filter(|reopening| !reopening.is_empty())
    {
        Some(reopening) => {
            let mut addresses = Some(addresses);
            reopening
                .into_iter()
                .map(|window| {
                    let content =
                        WindowContent::Reopen(window, addresses.take().unwrap_or_default());
                    shell::window(browser, false, content)
                })
                .collect()
        }
        None => vec![shell::window(browser, false, seeded(addresses))],
    }
}

rikka_app::mimalloc!();

fn main() -> rikka_app::Result<()> {
    let boot = Boot::begin();
    let launcher = Rc::clone(&boot);
    let ran = App::new("rikka-browser")
        .i18n(rikka_i18n::embedded!())
        .windows(move |open: LaunchRequest| open_windows(&launcher, open))
        .run();
    if let Some(browser) = boot.ready() {
        browser.finish();
    }
    drop(boot);
    rikka_web::wait_for_closed_runtimes();
    ran
}
