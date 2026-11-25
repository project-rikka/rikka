use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock};

use rikka_app::prelude::*;
use rikka_ui::Element;
use rikka_widgets::{column, row};

use crate::accounts;
use crate::launcher::{Launcher, Page};
use crate::pictures;
use crate::runs::Activity;

const SIDEBAR: f32 = 240.0;
const DESTINATION: f32 = 40.0;
const RECENT: usize = 6;
const LOGO_SIZE: f32 = 36.0;
const LOGO_RADIUS: f32 = 10.0;

static LOGO: LazyLock<Arc<[u8]>> =
    LazyLock::new(|| Arc::from(&include_bytes!(env!("RIKKA_LAUNCHER_LOGO"))[..]));

pub(crate) fn view(launcher: &Launcher) -> impl View {
    let current = match launcher.page {
        Page::Project => Page::Discover,
        Page::Instance => Page::Library,
        page => page,
    };
    let recent: Vec<Element> = launcher
        .instances
        .iter()
        .take(RECENT)
        .map(|instance| {
            recent_line(
                instance.id.clone(),
                instance.config.name.clone(),
                instance.icon().map(Path::to_path_buf),
                launcher.runs.activity(&instance.id),
                launcher.page == Page::Instance
                    && launcher.selected.as_deref() == Some(instance.id.as_str()),
            )
        })
        .collect();
    column! {
        row! {
            image(ImageSource::Bytes(Arc::clone(&LOGO)))
                .fit(ImageFit::Cover)
                .radius(LOGO_RADIUS)
                .size(Length::px(LOGO_SIZE), Length::px(LOGO_SIZE));
            column! {
                text(tr!("launcher.brand")).label().bold().no_wrap();
                text(tr!("launcher.brand-caption")).caption().muted().no_wrap();
            }
            .gap(Space::HAIR);
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .padding(Inset::edges(Space::SNUG, Space::SNUG, Space::ROOMY, Space::SNUG));
        destination(MaterialSymbol::HOME, tr!("launcher.nav.library"), Page::Library, current);
        destination(MaterialSymbol::EXPLORE, tr!("launcher.nav.discover"), Page::Discover, current);
        destination(MaterialSymbol::CHECKROOM, tr!("launcher.nav.skins"), Page::Skins, current);
        destination(MaterialSymbol::SETTINGS, tr!("launcher.nav.settings"), Page::Settings, current);
        if !recent.is_empty() {
            text(tr!("launcher.nav.recent"))
                .caption()
                .muted()
                .semibold()
                .margin(Inset::edges(Space::LOOSE, Space::SNUG, Space::TIGHT, Space::SNUG));
        }
        column_of(recent).gap(Space::HAIR).fill_width();
        spacer().grow(1.0);
        accounts::sidebar_card(launcher);
    }
    .gap(Space::HAIR)
    .padding(Inset::all(Space::NORMAL))
    .width(SIDEBAR)
    .fill_height()
    .background(palette().surface)
}

fn destination(symbol: MaterialSymbol, title: Label, page: Page, current: Page) -> Element {
    let chosen = page == current;
    let tint = match chosen {
        true => palette().accent,
        false => palette().text,
    };
    let glyph = icon(symbol).extent(Length::px(20.0)).color(tint);
    container(
        row! {
            match chosen {
                true => glyph.filled(),
                false => glyph,
            };
            text(title).label().medium().color(tint).no_wrap();
        }
        .gap(Space::NORMAL)
        .align_items(Align::Center),
    )
    .padding(Inset::xy(Space::NORMAL, Space::SNUG))
    .radius(Corner::Panel)
    .background(match chosen {
        true => palette().accent.with_alpha(0.14),
        false => Color::TRANSPARENT,
    })
    .on_press(move |launcher: &mut Launcher| launcher.go(page))
    .height(DESTINATION)
    .fill_width()
    .any()
}

fn recent_line(
    id: String,
    name: String,
    picture: Option<PathBuf>,
    activity: Activity,
    chosen: bool,
) -> Element {
    let dot = match activity {
        Activity::Idle => None,
        Activity::Preparing(_) => Some(palette().warning),
        Activity::Running => Some(palette().success),
    };
    container(
        row! {
            pictures::framed(
                picture.map(|path| ImageSource::file(Arc::from(path.as_path()))),
                MaterialSymbol::SPORTS_ESPORTS,
                24.0,
                6.0,
            );
            text(name).label().no_wrap().grow(1.0).min_width(0.0);
            dot.map(|tint| {
                container(spacer())
                    .background(tint)
                    .radius(4.0)
                    .size(Length::px(8.0), Length::px(8.0))
            });
        }
        .gap(Space::SNUG)
        .align_items(Align::Center),
    )
    .padding(Inset::xy(Space::NORMAL, Space::TIGHT))
    .radius(Corner::Item)
    .background(match chosen {
        true => palette().accent.with_alpha(0.10),
        false => Color::TRANSPARENT,
    })
    .on_press(move |launcher: &mut Launcher| launcher.open_instance(id.clone()))
    .fill_width()
    .any()
}
