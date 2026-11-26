mod render;

use std::rc::Rc;
use std::sync::Arc;

use launcher_auth::{Cape, Profile, SkinVariant};
use rikka_app::prelude::*;
use rikka_dialog::{Chooser, ChooserResponse, Filter};
use rikka_ui::Element;
use rikka_widgets::{column, row};

use crate::launcher::Launcher;
use crate::load::{Load, loading};
use crate::pictures;
use crate::services::blocking;
use crate::widgets::{heading, labelled, lift};

use render::Rendered;

const FACE_FACTOR: u32 = 8;
const BODY_FACTOR: u32 = 12;
const CAPE_FACTOR: u32 = 8;
const CAPE_TILE: f32 = 120.0;
const VARIANTS: [SkinVariant; 2] = [SkinVariant::Classic, SkinVariant::Slim];

#[derive(Default)]
pub(crate) struct Skins {
    account: Option<String>,
    profile: Load<Profile>,
    busy: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Part {
    Face,
    Body(SkinVariant),
    Cape,
}

fn rendered(url: &str, part: Part) -> Option<ImageSource> {
    let owned = url.to_owned();
    let picture = fetch_blocking(("launcher-skin", owned.clone(), part), move || {
        let bytes = pictures::download(&owned).map_err(|error| error.to_string())?;
        match part {
            Part::Face => render::face(&bytes, FACE_FACTOR),
            Part::Body(variant) => render::body(&bytes, variant, BODY_FACTOR),
            Part::Cape => render::cape(&bytes, CAPE_FACTOR),
        }
    });
    let Rendered {
        width,
        height,
        pixels,
    } = picture.ready()?.clone();
    Some(ImageSource::Rgba {
        width,
        height,
        pixels,
    })
}

pub(crate) fn face(launcher: &Launcher, account: &str) -> Option<ImageSource> {
    if launcher.skins.account.as_deref() != Some(account) {
        return None;
    }
    let skin = launcher.skins.profile.ready()?.active_skin()?;
    rendered(&skin.url, Part::Face)
}

type Changed = Result<Profile, String>;

impl Launcher {
    pub(crate) fn load_skins(&mut self) {
        let (Some(authenticator), Some(account)) = (
            self.authenticator.clone(),
            self.accounts.active().map(|account| account.id.clone()),
        ) else {
            return;
        };
        let same = self.skins.account.as_deref() == Some(account.as_str());
        match (same, &self.skins.profile) {
            (true, Load::Ready(_) | Load::Loading) => return,
            (true, Load::Idle | Load::Failed(_)) => {}
            (false, _) => self.skins.profile = Load::Loading,
        }
        self.skins.account = Some(account.clone());
        spawn_keyed(
            "launcher-skins",
            async move {
                let session = launcher_auth::session(&authenticator, &account)
                    .await
                    .map_err(|error| error.to_string())?;
                authenticator
                    .services()
                    .profile(session.access_token())
                    .await
                    .map_err(|error| error.to_string())
            },
            |launcher: &mut Launcher, found: Changed| {
                launcher.skins.profile = match found {
                    Ok(profile) => Load::Ready(profile),
                    Err(error) => Load::Failed(error),
                };
            },
        );
    }

    fn change_profile<F>(
        &mut self,
        change: impl FnOnce(Arc<launcher_auth::Authenticator>, String) -> F + Send + 'static,
    ) where
        F: std::future::Future<Output = launcher_auth::Result<Profile>> + Send + 'static,
    {
        let (Some(authenticator), Some(account)) = (
            self.authenticator.clone(),
            self.accounts.active().map(|account| account.id.clone()),
        ) else {
            return;
        };
        self.skins.busy = true;
        spawn_keyed(
            "launcher-skin-change",
            async move {
                let session = launcher_auth::session(&authenticator, &account)
                    .await
                    .map_err(|error| error.to_string())?;
                change(authenticator, session.access_token().to_owned())
                    .await
                    .map_err(|error| error.to_string())
            },
            |launcher: &mut Launcher, changed: Changed| {
                launcher.skins.busy = false;
                match changed {
                    Ok(profile) => launcher.skins.profile = Load::Ready(profile),
                    Err(error) => launcher.fail(error),
                }
            },
        );
    }

    fn upload_skin(&mut self, variant: SkinVariant) {
        let chooser = Chooser::open(tr!("launcher.skins.upload-title").to_string())
            .parent(window_identifier())
            .filter(
                Filter::named(tr!("launcher.skins.upload-filter").to_string())
                    .mime_type("image/png")
                    .pattern("*.png"),
            );
        spawn_blocking(
            move || -> Result<Option<Vec<u8>>, String> {
                let ChooserResponse::Files(files) =
                    chooser.run().map_err(|error| error.to_string())?
                else {
                    return Ok(None);
                };
                let Some(file) = files.first() else {
                    return Ok(None);
                };
                std::fs::read(file)
                    .map(Some)
                    .map_err(|error| error.to_string())
            },
            move |launcher: &mut Launcher, picked: Result<Option<Vec<u8>>, String>| match picked {
                Ok(Some(png)) => launcher.change_profile(move |authenticator, token| async move {
                    authenticator
                        .services()
                        .upload_skin(&token, variant, png)
                        .await
                }),
                Ok(None) => {}
                Err(error) => launcher.fail(error),
            },
        );
    }

    fn set_variant(&mut self, variant: SkinVariant) {
        let Some(skin) = self
            .skins
            .profile
            .ready()
            .and_then(|profile| profile.active_skin())
            .cloned()
        else {
            return;
        };
        if skin.variant == variant {
            return;
        }
        self.change_profile(move |authenticator, token| async move {
            let png = blocking(move || pictures::download(&skin.url))
                .await
                .map_err(launcher_auth::Error::Network)?
                .to_vec();
            authenticator
                .services()
                .upload_skin(&token, variant, png)
                .await
        });
    }

    fn reset_skin(&mut self) {
        self.change_profile(|authenticator, token| async move {
            authenticator.services().reset_skin(&token).await
        });
    }

    fn show_cape(&mut self, cape: Option<String>) {
        self.change_profile(move |authenticator, token| async move {
            match cape {
                Some(cape) => authenticator.services().show_cape(&token, &cape).await,
                None => authenticator.services().hide_cape(&token).await,
            }
        });
    }
}

fn variant_label(variant: SkinVariant) -> Label {
    match variant {
        SkinVariant::Classic => tr!("launcher.skins.classic"),
        SkinVariant::Slim => tr!("launcher.skins.slim"),
    }
}

fn cape_tile(cape: Option<&Cape>, chosen: bool, busy: bool) -> Element {
    let picture = match cape {
        Some(cape) => match rendered(&cape.url, Part::Cape) {
            Some(source) => image(source).fit(ImageFit::Contain).fill().any(),
            None => spinner().extent(Length::px(18.0)).any(),
        },
        None => icon(MaterialSymbol::BLOCK)
            .extent(Length::px(28.0))
            .color(palette().text_muted)
            .any(),
    };
    let label = match cape {
        Some(cape) if !cape.alias.is_empty() => Label::from(cape.alias.clone()),
        Some(_) => tr!("launcher.skins.cape"),
        None => tr!("launcher.skins.no-cape"),
    };
    let chosen_cape = cape.map(|cape| cape.id.clone());
    lift(
        container(
            column! {
                container(picture)
                    .center()
                    .padding(Inset::all(Space::SNUG))
                    .background(palette().surface_variant)
                    .radius(Corner::Item)
                    .height(Length::px(CAPE_TILE))
                    .fill_width();
                text(label).caption().semibold().no_wrap().align(TextAlign::Center).fill_width();
            }
            .gap(Space::SNUG)
            .fill_width(),
        )
        .padding(Inset::all(Space::SNUG))
        .radius(Corner::Panel)
        .border(
            2.0,
            match chosen {
                true => palette().accent,
                false => Color::TRANSPARENT,
            },
        )
        .on_press(move |launcher: &mut Launcher| {
            if !busy && !chosen {
                launcher.show_cape(chosen_cape.clone());
            }
        })
        .width(Length::px(CAPE_TILE)),
    )
}

fn profile_view(profile: &Profile, busy: bool) -> Element {
    let skin = profile.active_skin().cloned();
    let variant = skin
        .as_ref()
        .map_or(SkinVariant::Classic, |skin| skin.variant);
    let preview = match &skin {
        Some(skin) => match rendered(&skin.url, Part::Body(skin.variant)) {
            Some(source) => image(source).fit(ImageFit::Contain).fill().any(),
            None => spinner().extent(Length::px(24.0)).any(),
        },
        None => icon(MaterialSymbol::PERSON).extent(Length::px(64.0)).any(),
    };
    let active_cape = profile.active_cape().map(|cape| cape.id.clone());
    let capes: Rc<[Cape]> = profile.capes.clone().into();
    let mut tiles = vec![cape_tile(None, active_cape.is_none(), busy)];
    tiles.extend(capes.iter().map(|cape| {
        cape_tile(
            Some(cape),
            active_cape.as_deref() == Some(cape.id.as_str()),
            busy,
        )
    }));
    row! {
        container(preview)
            .center()
            .padding(Inset::all(Space::LOOSE))
            .surface()
            .radius(Corner::Panel)
            .border(1.0, palette().outline)
            .size(Length::px(280.0), Length::px(440.0));
        column! {
            text(profile.name.clone()).title().bold();
            heading(tr!("launcher.skins.model"));
            segmented(VARIANTS.iter().position(|shown| *shown == variant).unwrap_or_default())
                .segments(VARIANTS.map(variant_label))
                .enabled(!busy && skin.is_some())
                .on_select(|launcher: &mut Launcher, index: usize| {
                    if let Some(variant) = VARIANTS.get(index) {
                        launcher.set_variant(*variant);
                    }
                });
            row! {
                button(labelled(MaterialSymbol::UPLOAD, tr!("launcher.skins.upload")))
                    .primary()
                    .loading(busy)
                    .on_press(move |launcher: &mut Launcher| launcher.upload_skin(variant));
                button(labelled(MaterialSymbol::RESTART_ALT, tr!("launcher.skins.reset")))
                    .ghost()
                    .enabled(!busy)
                    .on_press(Launcher::reset_skin);
            }
            .gap(Space::SNUG);
            heading(tr!("launcher.skins.capes"));
            row_of(tiles).gap(Space::SNUG);
        }
        .gap(Space::NORMAL)
        .grow(1.0)
        .min_width(0.0);
    }
    .gap(Space::WIDE)
    .fill_width()
    .any()
}

pub(crate) fn view(launcher: &Launcher) -> Element {
    let body = match launcher.accounts.active() {
        None => status_page(MaterialSymbol::CHECKROOM, tr!("launcher.skins.signed-out"))
            .action(
                button(text(tr!("launcher.accounts.sign-in")))
                    .primary()
                    .on_press(Launcher::begin_sign_in),
            )
            .fill()
            .any(),
        Some(_) => loading(&launcher.skins.profile, |profile: &Profile| {
            profile_view(profile, launcher.skins.busy)
        }),
    };
    scrollable(
        column! {
            text(tr!("launcher.skins.title")).title().bold();
            body;
        }
        .gap(Space::LOOSE)
        .padding(Inset::all(Space::WIDE))
        .fill_width(),
    )
    .fill()
    .any()
}
